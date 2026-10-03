use std::sync::Arc;
use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio::process::Command;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};

// SymbolsTool extracted from core/src/tools/mod.rs (tools/read/symbols.rs)
pub struct SymbolsTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

/// Nombres bajo los que puede estar el binario de ast-grep.
///
/// `sg` es el nombre corto clasico; `ast-grep` es el que instala el paquete
/// de Arch (`ast-grep`, binario en `/usr/bin/ast-grep`). Intentamos los dos
/// porque solo uno existe segun como se instalo, y antes la tool hardcodeaba
/// `sg`: en una maquina con `ast-grep` instalado caia siempre al fallback.
const AST_GREP_BINARIES: &[&str] = &["sg", "ast-grep"];

/// Keywords de definicion por lenguaje, para el fallback regex.
const DEF_KEYWORDS: &[&str] = &[
    "fn", "func", "def", "class", "struct", "enum", "trait", "interface",
    "type", "const", "let", "var", "funcs", "function", "method", "impl",
];

#[async_trait]
impl Tool for SymbolsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "symbols".to_string(),
            description: "Search for function/class/method definitions in code. Uses ast-grep (sg or ast-grep) if available, falls back to regex. Accepts either a symbol NAME (e.g. `main`) or a structural ast-grep PATTERN (e.g. `def $FUNC($$$ARGS):`).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Symbol name, or an ast-grep pattern with $METAVARS"},
                    "path": {"type": "string", "default": ".", "description": "Directory to search in"},
                    "limit": {"type": "integer", "default": 50}
                },
                "required": ["query"]
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Filesystem]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Plan]
    }
    async fn execute(
        &self,
        _ctx: &crate::ExecuteContext,
        args: Value,
    ) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("missing 'query' (or empty)")?
            .trim();
        if query.is_empty() {
            return Err("missing 'query' (or empty)".to_string());
        }
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .clamp(1, 500) as usize;
        let search_dir = {
            let raw = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let p = self.workspace_root.join(raw);
            p.canonicalize()
                .map_err(|e| format!("path: {e}"))?
        };

        // ── 1. ast-grep ───────────────────────────────────────────────────
        //
        // Solo tiene sentido si el query es un PATRON estructural (con
        // metavariables `$X`). Para un nombre simple (`main`) ast-grep no
        // aporta nada, asi que se salta directo al regex y no paga el
        // costo de spawnear un proceso.
        if query.contains('$') {
            for bin in AST_GREP_BINARIES {
                if let Some(out) = run_ast_grep(bin, query, &search_dir).await {
                    return Ok(out);
                }
            }
        }

        // ── 2. fallback regex ─────────────────────────────────────────────
        //
        // Antes el pattern era `\b(fn|def|...)\s+{query}\b` con
        // `regex::escape(query)`: el query entraba como LITERAL, asi que
        // un patron con metavariables (`def $FUNC($$$ARGS)`) jamas podia
        // matchear y la tool devolvia siempre "no symbols matching".
        //
        // Ahora hay dos caminos:
        //   - query con metavariables -> se convierte en un regex de
        //     "keyword + firma": los metavars `$FUNC` / `$$$ARGS` se
        //     convierten en `[\w$]+` y el resto se escapa.
        //   - query simple -> se busca el nombre y se muestra la linea
        //     completa (que es lo que el usuario quiere ver: la firma).
        let pattern = if query.contains('$') {
            // `def $FUNC($$$ARGS):` -> `def\s+[\w$]+\s*\(`
            let sig = metavar_pattern(query);
            format!(
                r"(?i)\b(?:{keywords})\s+{sig}",
                keywords = DEF_KEYWORDS.join("|"),
            )
        } else {
            // Nombre simple: matchea la palabra en cualquier contexto de
            // definicion. Se mantiene el lookaround de keyword para no
            // devolver cada mencion en un comentario.
            format!(
                r"(?i)\b(?:{keywords})\s+[\w:]*{name}\b",
                keywords = DEF_KEYWORDS.join("|"),
                name = regex::escape(query),
            )
        };
        let re = regex::Regex::new(&pattern).map_err(|e| format!("regex: {e}"))?;
        let mut results = Vec::new();
        grep_walk_dir(&search_dir, &re, limit, &mut results)?;
        if results.is_empty() {
            Ok(format!("no symbols matching '{query}' found"))
        } else {
            Ok(results.join("\n"))
        }
    }
}

/// Corre ast-grep con el binario dado. `None` si el binario no existe o
/// falla — en ambos casos se cae al fallback regex.
async fn run_ast_grep(bin: &str, pattern: &str, dir: &std::path::Path) -> Option<String> {
    let out = Command::new(bin)
        .arg("--pattern")
        .arg(pattern)
        .arg("--json")
        .arg(".")
        .current_dir(dir)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if stdout.trim().is_empty() {
        return None;
    }
    // ast-grep --json devuelve una lista de matches. El cliente espera
    // texto legible, asi que se aplana a lineas `file:line: text`.
    Some(format_ast_grep_json(&stdout))
}

/// Convierte la salida JSON de ast-grep en lineas `archivo:linea: texto`.
///
/// Un parseo a mano con `serde_json::Value`: la forma de ast-grep cambia
/// entre versiones y no queremos atar el binario a una struct. Si el JSON
/// no parsea se devuelve el stdout crudo, que sigue siendo util.
fn format_ast_grep_json(raw: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_string();
    };
    let Some(items) = v.as_array() else {
        return raw.to_string();
    };
    let mut lines = Vec::new();
    for item in items {
        let file = item
            .get("file")
            .and_then(|f| f.as_str())
            .unwrap_or("?");
        let line_no = item
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0)
            + 1;
        let text = item
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .trim();
        lines.push(format!("{file}:{line_no}: {text}"));
    }
    lines.join("\n")
}

/// Traduce un patron de ast-grep a un regex de Rust.
///
/// Recorre el query caracter a caracter en vez de hacer replaces con un
/// marcador: los reemplazos con sentinel se pisan entre si (`$$$` vs `$`)
/// y perdian el nombre del metavar — `$FUNC` terminaba en `METAFUNC` y el
/// regex pedia un metavar llamado "FUNC" en vez de cualquier identificador.
///
///   - `$X`, `$$X`, `$$$X` (con o sin nombre) -> `[\w$]+`
///   - cualquier otra cosa -> `regex::escape`
///
/// No cubre toda la gramatica de ast-grep (no maneja `$` como ancla de
/// bloque ni los `context`), pero cubre el caso que reportaba la revision:
/// buscar definiciones por firma.
fn metavar_pattern(query: &str) -> String {
    let chars: Vec<char> = query.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            // Todos los `$` seguidos (1, 2 o 3) son el prefijo del metavar.
            while i < chars.len() && chars[i] == '$' {
                i += 1;
            }
            // El nombre del metavar, si lo tiene, no se busca: se
            // reemplaza entero por la clase de caracteres.
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push_str(r"[\w$]+");
        } else {
            let mut lit = String::new();
            lit.push(chars[i]);
            out.push_str(&regex::escape(&lit));
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sandbox(readable: Vec<String>) -> Arc<tokio::sync::RwLock<Box<dyn SandboxConfig>>> {
        Arc::new(tokio::sync::RwLock::new(Box::new(TestSandbox {
            enabled: true,
            readable,
        })))
    }

    struct TestSandbox {
        enabled: bool,
        readable: Vec<String>,
    }
    impl SandboxConfig for TestSandbox {
        fn enabled(&self) -> bool {
            self.enabled
        }
        fn is_writable(&self, _: &std::path::Path) -> bool {
            false
        }
        fn is_readable(&self, path: &std::path::Path) -> bool {
            self.readable
                .iter()
                .any(|r| path.starts_with(std::path::PathBuf::from(r)))
        }
    }

    fn tool_for(dir: &std::path::Path) -> SymbolsTool {
        SymbolsTool {
            workspace_root: dir.to_path_buf(),
            sandbox: make_sandbox(vec![dir.to_string_lossy().to_string()]),
        }
    }

    // ── metavar_pattern ──────────────────────────────────────────────────

    #[test]
    fn metavar_pattern_convierte_los_metavars() {
        // El bug reportado: `def $FUNC($$$ARGS):` se escapaba entero y
        // no podia matchear nunca.
        let re = metavar_pattern("def $FUNC($$$ARGS):");
        assert!(
            re.contains("[\\w$]+"),
            "los metavars deben volverse una clase de caracteres: {re}"
        );
        // El `$` que queda es el de la clase de caracteres, que es
        // intencional (nombres de shell/PHP empiezan con $). Lo que no
        // debe quedar es el nombre del metavar.
        assert!(!re.contains("FUNC"), "el nombre del metavar no debe quedar: {re}");
        assert!(!re.contains("ARGS"), "el nombre del metavar no debe quedar: {re}");
        // Y debe compilar como regex de Rust.
        assert!(regex::Regex::new(&format!("def\\s+{re}")).is_ok());
    }

    #[test]
    fn metavar_pattern_escapa_el_resto() {
        let re = metavar_pattern("class $A");
        assert!(regex::Regex::new(&re).is_ok(), "no debe romper el regex: {re}");
    }

    // ── fallback sobre ficheros reales ───────────────────────────────────

    #[tokio::test]
    async fn encuentra_def_en_python_por_nombre() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("app.py"),
            "import os\n\ndef procesar(datos):\n    return datos\n\nclass Motor:\n    pass\n",
        )
        .unwrap();
        let out = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({"query": "procesar"}),
            )
            .await
            .expect("execute");
        assert!(out.contains("def procesar"), "debe hallar la firma: {out}");
    }

    #[tokio::test]
    async fn encuentra_def_en_python_por_patron_de_metavars() {
        // El caso exacto del informe: con el fallback viejo devolvia
        // "no symbols matching" aunque el fichero tuviera funciones.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("app.py"),
            "def procesar(datos):\n    return datos\n\ndef otra(x, y):\n    return x\n",
        )
        .unwrap();
        let out = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({"query": "def $FUNC($$$ARGS)"}),
            )
            .await
            .expect("execute");
        assert!(
            !out.contains("no symbols matching"),
            "no debe decir que no encuentra nada: {out}"
        );
        assert!(out.contains("def procesar"), "debe hallar la firma: {out}");
    }

    #[tokio::test]
    async fn encuentra_rust_por_nombre() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() {}\nfn helper() {}\n",
        )
        .unwrap();
        let out = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({"query": "main"}),
            )
            .await
            .expect("execute");
        assert!(out.contains("fn main"), "debe hallar la firma: {out}");
    }

    #[tokio::test]
    async fn query_inexistente_no_encuentra() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.py"), "def x():\n    pass\n").unwrap();
        let out = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({"query": "no_existe_este_simbolo"}),
            )
            .await
            .expect("execute");
        assert!(out.contains("no symbols matching"), "{out}");
    }

    #[tokio::test]
    async fn query_vacio_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({"query": "  "}),
            )
            .await
            .expect_err("debe fallar");
        assert!(err.contains("query"), "{err}");
    }

    // ── format_ast_grep_json ─────────────────────────────────────────────

    #[test]
    fn aplana_el_json_de_ast_grep() {
        let raw = r#"[
          {"file":"app.py","text":"def foo():","range":{"start":{"line":3,"column":0}}}
        ]"#;
        let out = format_ast_grep_json(raw);
        assert_eq!(out, "app.py:4: def foo():");
    }

    #[test]
    fn json_invalido_devuelve_el_raw() {
        let raw = "no soy json";
        assert_eq!(format_ast_grep_json(raw), raw);
    }
}

#[cfg(test)]
mod parser_tests {
    use super::metavar_pattern;

    #[test]
    fn query_simple_se_escapa() {
        assert_eq!(metavar_pattern("main"), "main");
    }

    #[test]
    fn parentesis_se_escapan() {
        let re = metavar_pattern("def $FUNC()");
        assert!(re.contains(r"\("), "el parentesis debe quedar escapado: {re}");
        assert!(regex::Regex::new(&re).is_ok());
    }
}
