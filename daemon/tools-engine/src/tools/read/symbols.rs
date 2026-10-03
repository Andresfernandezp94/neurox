use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{Mode, Tool, ToolCategory, ToolSpec};

/// Busqueda de definiciones por nombre, con deteccion de lenguaje.
///
/// EP-2026-10-03. Este modulo se reescribio entero. La version anterior
/// aceptaba un patron ast-grep en crudo y, si ast-grep no podia resolverlo,
/// caia a un regex que INTENTABA traducir ese patron. Esa traduccion es
/// intrinsecamente incompleta —ast-grep exige que el patron sea codigo
/// parseable— y se manifesto como una cadena de fallos: el regex pedia
/// `def def`, luego `$$$` de repeticion no casaba listas de argumentos, luego
/// `$FUNC` se confundia con la palabra clave `func`. Cada arreglo destapaba
/// el siguiente.
///
/// El cambio de fondo: **el agente no escribe patrones**. Escribe un NOMBRE y
/// la tool decide el patron, porque el lenguaje del fichero es informacion que
/// el agente no tiene por que conocer. Que `def $S($$$A): $$$B` sea valido y
/// `func $S($$$A) $R { $$$B }` haga falta en Go son detalles de esta tabla, no
/// del prompt.
pub struct SymbolsTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

/// Nombres bajo los que puede estar el binario de ast-grep.
///
/// `sg` es el nombre corto clasico; en Arch el paquete `ast-grep` instala el
/// binario como `ast-grep`. Se prueban los dos porque solo uno existe segun
/// como se instalo.
const AST_GREP_BINARIES: &[&str] = &["ast-grep", "sg"];

/// Un patron de definicion para un lenguaje.
///
/// El metavar SIEMPRE se llama `SYMBOL`, en todos los patrones y todos los
/// lenguajes: asi `metaVariables.single.SYMBOL` se lee igual sin importar el
/// idioma, y anadir un lenguaje no obliga a tocar el lector.
struct DefPattern {
    /// Grupo de la definicion, para el parametro `kind`.
    kind: &'static str,
    pattern: &'static str,
}

/// Un lenguaje: como se le llama a ast-grep, que extensiones lo identifican y
/// como se declaran sus funciones, clases y tipos.
struct LangSpec {
    lang: &'static str,
    extensions: &'static [&'static str],
    patterns: &'static [DefPattern],
}

/// Tabla de lenguajes.
///
/// Cada entrada esta VERIFICADA contra el binario: hay un test
/// (`la_tabla_encuentra_lo_que_declara`) que crea un fichero por lenguaje con
/// un simbolo conocido y comprueba que la tabla lo encuentra. Si alguien anade
/// un patron que no casa, ese test falla.
///
/// Notas de por que los patrones son como son, sacadas de la guia de ast-grep
/// ("Pattern code must be valid code that tree-sitter can parse") y de
/// comprobar cada uno contra el binario:
///
/// - Un `def` de Python necesita cuerpo: `def f(a):` a secas no casa, hace
///   falta `$$$BODY`. Esto era el bug que reportaba la revision: quitar el
///   dos puntos "arreglaba" el patron por casualidad, no por reason.
/// - Go lleva tipo de retorno entre los parentesis y la llave, asi que el
///   patron tiene `$RET`: `func f($$$A) $R { $$$B }`.
/// - Una clase de Python puede no llevar parentesis (`class C:`) o llevarlos
///   (`class C(Base):`), asi que hacen falta los dos patrones.
/// - Ruby cierra con `end`, luego el patron necesita saltos de linea reales.
/// - La mayoria de los lenguajes tiene DOS formas validas para una misma
///   definicion (con y sin tipo de retorno), y hacen falta los dos
///   patrones: `fn f() {}` y `fn f() -> u32 {}` no los cubre uno solo. Por
///   eso el test de la tabla prueba las dos variantes de cada lenguaje.
const LANGUAGES: &[LangSpec] = &[
    LangSpec {
        lang: "python",
        extensions: &["py", "py3", "pyi"],
        patterns: &[
            DefPattern { kind: "function", pattern: "def $SYMBOL($$$ARGS): $$$BODY" },
            DefPattern { kind: "class", pattern: "class $SYMBOL: $$$BODY" },
            DefPattern { kind: "class", pattern: "class $SYMBOL($$$BASE): $$$BODY" },
        ],
    },
    LangSpec {
        lang: "rust",
        extensions: &["rs"],
        patterns: &[
            DefPattern { kind: "function", pattern: "fn $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern { kind: "function", pattern: "fn $SYMBOL($$$ARGS) -> $RET { $$$BODY }" },
            // `async fn` es otro nodo para tree-sitter: sin estos dos, medio
            // repo Rust (todo lo async) salia como "no encontrado".
            DefPattern { kind: "function", pattern: "async fn $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern {
                kind: "function",
                pattern: "async fn $SYMBOL($$$ARGS) -> $RET { $$$BODY }",
            },
            DefPattern { kind: "type", pattern: "struct $SYMBOL { $$$BODY }" },
            DefPattern { kind: "type", pattern: "struct $SYMBOL($$$FIELDS);" },
            DefPattern { kind: "type", pattern: "enum $SYMBOL { $$$BODY }" },
            DefPattern { kind: "type", pattern: "trait $SYMBOL { $$$BODY }" },
            DefPattern { kind: "impl", pattern: "impl $SYMBOL { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "javascript",
        extensions: &["cjs", "js", "mjs", "jsx"],
        patterns: &[
            DefPattern { kind: "function", pattern: "function $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
            DefPattern { kind: "value", pattern: "const $SYMBOL = $VAL" },
        ],
    },
    LangSpec {
        lang: "typescript",
        extensions: &["ts", "cts", "mts"],
        patterns: &[
            DefPattern {
                kind: "function",
                pattern: "function $SYMBOL($$$ARGS): $RET { $$$BODY }",
            },
            DefPattern {
                kind: "function",
                pattern: "function $SYMBOL($$$ARGS) { $$$BODY }",
            },
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
            DefPattern { kind: "type", pattern: "interface $SYMBOL { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "tsx",
        extensions: &["tsx"],
        patterns: &[
            DefPattern { kind: "function", pattern: "function $SYMBOL($$$ARGS): $RET { $$$BODY }" },
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "go",
        extensions: &["go"],
        patterns: &[
            // El tipo de retorno va entre `)` y `{`. Sin el, no casa.
            DefPattern { kind: "function", pattern: "func $SYMBOL($$$ARGS) $RET { $$$BODY }" },
            DefPattern { kind: "function", pattern: "func $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern { kind: "type", pattern: "type $SYMBOL struct { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "java",
        extensions: &["java"],
        patterns: &[
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
            // Cubre metodos: `int metodo(int a) { ... }`. El tipo de retorno
            // es un identificador, asi que no lleva metavar.
            DefPattern { kind: "function", pattern: "$RET $SYMBOL($$$ARGS) { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "ruby",
        extensions: &["rb", "rbw"],
        patterns: &[
            DefPattern { kind: "function", pattern: "def $SYMBOL($$$ARGS)\n  $$$BODY\nend" },
            DefPattern { kind: "class", pattern: "class $SYMBOL\n  $$$BODY\nend" },
        ],
    },
    LangSpec {
        lang: "php",
        extensions: &["php"],
        patterns: &[
            DefPattern { kind: "function", pattern: "function $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern { kind: "function", pattern: "function $SYMBOL($$$ARGS): $RET { $$$BODY }" },
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "csharp",
        extensions: &["cs"],
        patterns: &[
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
            DefPattern { kind: "function", pattern: "$RET $SYMBOL($$$ARGS) { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "cpp",
        extensions: &["cpp", "cxx", "cc", "hpp"],
        patterns: &[
            DefPattern { kind: "function", pattern: "$RET $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "c",
        extensions: &["c", "h"],
        patterns: &[
            DefPattern { kind: "function", pattern: "$RET $SYMBOL($$$ARGS) { $$$BODY }" },
        ],
    },
    LangSpec {
        lang: "kotlin",
        extensions: &["kt", "kts"],
        patterns: &[
            DefPattern { kind: "function", pattern: "fun $SYMBOL($$$ARGS) { $$$BODY }" },
            DefPattern { kind: "function", pattern: "fun $SYMBOL($$$ARGS): $RET { $$$BODY }" },
            DefPattern { kind: "class", pattern: "class $SYMBOL { $$$BODY }" },
        ],
    },
];

/// Grupos de definicion que acepta el parametro `kind`.
const KINDS: &[&str] = &["function", "class", "type", "impl", "value"];

/// Directorios que no se recorren: ruido y lentitud.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", ".git", "venv", ".venv", "__pycache__",
    ".next", "vendor", "coverage",
];

/// Una definicion encontrada.
struct Hit {
    lang: String,
    file: String,
    line: usize,
    kind: &'static str,
    symbol: String,
    signature: String,
}

#[async_trait]
impl Tool for SymbolsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "symbols".to_string(),
            description: "Find where a function, class or type is DEFINED, by name. \
Detects the language of each file itself, so you do not need to know it: pass \
only the name (e.g. 'main', 'process_file') and optionally kind \
(function|class|type|impl|value). Works across mixed-language trees. \
Needs ast-grep for structural accuracy; without it, falls back to a text \
search and says so in the output."
.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The symbol NAME to find, e.g. 'main'. Do NOT pass an ast-grep pattern."
                    },
                    "path": {"type": "string", "default": ".", "description": "Directory to search in"},
                    "kind": {
                        "type": "string",
                        "enum": KINDS,
                        "description": "Restrict to a kind of definition. Omit to search all."
                    },
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
            .map(str::trim)
            .ok_or("missing 'query'")?;
        if query.is_empty() {
            return Err("missing 'query' (or empty)".to_string());
        }

        // Rechaza patrones en crudo en vez de adivinar. Antes se aceptaban y
        // se traducian a regex a ojo, con lo que un patron invalido para el
        // lenguaje del fichero devolvia "no symbols matching" sin decir por
        // que. Ahora el error dice exactamente que hacer.
        if query.contains('$') {
            return Err(format!(
                "'query' is a symbol NAME, not an ast-grep pattern: '{query}' \
contains '$'. Pass just the name (e.g. 'main'). If you really need a \
structural search, run ast-grep yourself through the shell tool."
            ));
        }
        if query.chars().any(char::is_whitespace) {
            return Err(format!(
                "'query' must be a single symbol name, but '{query}' has spaces"
            ));
        }

        let kind = args.get("kind").and_then(|v| v.as_str()).map(str::trim);
        if let Some(k) = kind {
            if !KINDS.contains(&k) {
                return Err(format!(
                    "unknown kind '{k}'. Valid: {}",
                    KINDS.join(", ")
                ));
            }
        }

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .clamp(1, 500) as usize;

        // `path` se valida contra el sandbox ANTES de tocar el arbol. Antes
        // hacia `workspace_root.join(raw)` a pelo, que no bloquea nada:
        // `Path::join` con una ruta absoluta descarta la base (o sea
        // `path: "/etc"` se aceptaba entero) y un `../` subia hasta el
        // padre del workspace. La tool no devuelve el cuerpo del fichero,
        // pero si las rutas y la primera linea de cada definicion que
        // encuentre ahi fuera. Mismo helper y mismo `writable: false` que
        // usan read_file/glob/grep/list_dir.
        let raw = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let readable = self.sandbox.read().await.readable_paths_resolved(&self.workspace_root);

        // 1) Chequeo lexico: `..`, rutas absolutas y las de `readable_paths`.
        let resolved = resolve_under_workspace(&self.workspace_root, raw, &readable, false)
            .map_err(|e| format!("path: {e}"))?;

        if !resolved.exists() {
            return Err(format!("path not found: {raw}"));
        }

        // 2) Segundo chequeo sobre la ruta YA canonicalizada. El de arriba es
        // lexico y no ve los symlinks: uno dentro del workspace que apunte
        // fuera pasaria el primero y el recorrido lo seguiria. Comparar
        // contra la raiz canonicalizada cierra ese caso.
        let search_dir = resolved.canonicalize().map_err(|e| format!("path: {e}"))?;
        let canon_ws = self
            .workspace_root
            .canonicalize()
            .unwrap_or_else(|_| self.workspace_root.clone());
        let dentro = search_dir.starts_with(&canon_ws)
            || readable.iter().any(|p| {
                p.canonicalize()
                    .map(|c| search_dir.starts_with(c))
                    .unwrap_or_else(|_| search_dir.starts_with(p))
            });
        if !dentro {
            return Err(format!(
                "path '{raw}' resolves outside workspace '{}' (not readable)",
                self.workspace_root.display()
            ));
        }

        // Un solo recorrido para saber que lenguajes hay. Luego solo se
        // invocan los patrones de los que de verdad aparecen: si nadie
        // escribe Kotlin, no se lanza ast-grep con los 3 patrones de Kotlin.
        let files_by_ext = collect_source_files(&search_dir, 4000);
        let present = detect_languages(&files_by_ext);

        if present.is_empty() {
            return Ok(format!(
                "no source files with a known language found under {}",
                search_dir.display()
            ));
        }

        let Some(bin) = find_ast_grep().await else {
            return Ok(text_fallback(&search_dir, query, kind, limit));
        };

        let mut hits = Vec::new();
        for lang in present {
            let patterns: Vec<&DefPattern> = lang
                .spec
                .patterns
                .iter()
                .filter(|p| kind.map(|k| p.kind == k).unwrap_or(true))
                .collect();
            for pat in patterns {
                hits.extend(
                    run_pattern(&bin, lang.spec.lang, pat, &lang.files, &query).await,
                );
            }
        }

        // Exacto primero; si no hay ninguno, parcial. Un nombre corto como
        // 'run' Matching prefijo de 'run_all' es lo que se busca cuando no
        // hay coincidencia exacta.
        let mut exact: Vec<Hit> = Vec::new();
        let mut partial: Vec<Hit> = Vec::new();
        for h in hits {
            if h.symbol == query {
                exact.push(h);
            } else if h.symbol.contains(query) {
                partial.push(h);
            }
        }
        let (mut found, fuzzy) = if exact.is_empty() {
            (partial, true)
        } else {
            (exact, false)
        };
        found.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        let truncated = found.len() > limit;
        let shown: Vec<&Hit> = found.iter().take(limit).collect();

        if shown.is_empty() {
            let kinds = kind.map(|k| format!(" of kind '{k}'")).unwrap_or_default();
            return Ok(format!(
                "no definition named '{query}'{kinds} in {} file(s) scanned \
(languages: {}).\n\
The files were searched structurally, so the symbol genuinely is not defined \
there. It may be defined elsewhere, called with a different name, imported \
from another module, or this may be a call site rather than a definition.",
                files_by_ext.values().map(Vec::len).sum::<usize>(),
                langs_summary(&files_by_ext),
            ));
        }

        let mut out = String::new();
        if fuzzy {
            out.push_str(&format!(
                "# no exact match for '{query}'; showing symbols that CONTAIN it\n"
            ));
        }
        for h in &shown {
            out.push_str(&format!(
                "{} | {}:{} | {} | {}\n",
                h.lang, h.file, h.line, h.kind, h.signature
            ));
        }
        if truncated {
            out.push_str(&format!(
                "\n[showing {} of {} matches; raise 'limit' to see more]\n",
                shown.len(),
                found.len()
            ));
        }
        Ok(out)
    }
}

/// Un lenguaje presente en el arbol, con sus ficheros.
struct PresentLang<'a> {
    spec: &'a LangSpec,
    files: Vec<PathBuf>,
}

/// Fija `lang -> spec`. `LANGUAGES` son datos, no logica: anadir un idioma es
/// anadir una entrada, y el test de cobertura lo detecta si el patron falla.
fn detect_languages(files_by_ext: &BTreeMap<String, Vec<PathBuf>>) -> Vec<PresentLang<'_>> {
    let mut out = Vec::new();
    for spec in LANGUAGES {
        let files: Vec<PathBuf> = spec
            .extensions
            .iter()
            .filter_map(|ext| files_by_ext.get(*ext))
            .flatten()
            .cloned()
            .collect();
        if !files.is_empty() {
            out.push(PresentLang { spec, files });
        }
    }
    out
}

fn langs_summary(files_by_ext: &BTreeMap<String, Vec<PathBuf>>) -> String {
    let mut parts = Vec::new();
    for spec in LANGUAGES {
        let n: usize = spec
            .extensions
            .iter()
            .filter_map(|e| files_by_ext.get(*e))
            .map(Vec::len)
            .sum();
        if n > 0 {
            parts.push(format!("{}: {}", spec.lang, n));
        }
    }
    parts.join(", ")
}

/// Un recorrido del arbol agrupando por extension. Corta en `max_files` para
/// que un arbol enorme no tumbe la tool: es preferible devolver menos a no
/// devolver nada.
fn collect_source_files(dir: &Path, max_files: usize) -> BTreeMap<String, Vec<PathBuf>> {
    let mut out: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let known: HashSet<&str> = LANGUAGES
        .iter()
        .flat_map(|l| l.extensions.iter().copied())
        .collect();
    let mut queue = vec![dir.to_path_buf()];
    let mut total = 0usize;
    while let Some(d) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                    continue;
                }
                queue.push(path);
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let Some(ext) = path.extension().map(|e| e.to_string_lossy().to_string()) else {
                continue;
            };
            if !known.contains(ext.as_str()) {
                continue;
            }
            out.entry(ext).or_default().push(path);
            total += 1;
            if total >= max_files {
                return out;
            }
        }
    }
    out
}

/// Busca el binario. Son 2 spawns como mucho (uno por nombre) POR LLAMADA,
/// lo cual se documenta porque antes de esto decia "cacheado por proceso" y
/// no habia ninguna cache. No se cachea: probeando son 2 procesos frente a
/// las decenas de invocaciones de `run_pattern` que hace el resto, asi que
/// la cache no compra nada que compense su complejidad. Si algun dia el
/// perfil muestra que si, el sitio es aqui.
async fn find_ast_grep() -> Option<String> {
    for bin in AST_GREP_BINARIES {
        let ok = Command::new(bin)
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(bin.to_string());
        }
    }
    None
}

/// Lanza un patron sobre un fichero y devuelve las definiciones cuyo simbolo
/// capturado case con la query.
///
/// Se pasa el `--lang` explicito: si se deja que ast-grep lo deduzca del
/// arbol, `--json` incluiria ficheros de otros lenguajes con el mismo patron.
async fn run_pattern(
    bin: &str,
    lang: &str,
    pat: &DefPattern,
    files: &[PathBuf],
    query: &str,
) -> Vec<Hit> {
    // Una invocacion por patron, con TODOS los ficheros del lenguaje, en
    // tandas: antes era una por fichero, que en un repo real son miles de
    // spawns (medido: 11s en 356 ficheros). Las tandas existen porque la
    // linea de comandos tiene limite de longitud.
    const BATCH: usize = 150;
    let mut hits = Vec::new();
    for batch in files.chunks(BATCH) {
        hits.extend(run_pattern_batch(bin, lang, pat, batch, query).await);
    }
    hits
}

async fn run_pattern_batch(
    bin: &str,
    lang: &str,
    pat: &DefPattern,
    files: &[PathBuf],
    query: &str,
) -> Vec<Hit> {
    let out = Command::new(bin)
        .args(["run", "--lang", lang, "--pattern", pat.pattern, "--json"])
        .args(files)
        .output()
        .await;

    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_slice::<Value>(&out.stdout) else {
        return Vec::new();
    };
    let Some(items) = json.as_array() else { return Vec::new() };

    let mut hits = Vec::new();
    for item in items {
        let symbol = item
            .get("metaVariables")
            .and_then(|m| m.get("single"))
            .and_then(|s| s.get("SYMBOL"))
            .and_then(|s| s.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        // Se admite exacto O parcial aqui, y quien llama distingue. Filtrar
        // solo por igualdad dejaba muerto el camino de las coincidencias
        // parciales: nunca llegaba nada que presenters como "CONTAIN it".
        if symbol.is_empty() || !symbol.contains(query) {
            continue;
        }
        let line = item
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0) as usize
            + 1;
        let file = item
            .get("file")
            .and_then(|f| f.as_str())
            .unwrap_or_default()
            .to_string();
        hits.push(Hit {
            lang: lang.to_string(),
            file,
            line,
            kind: pat.kind,
            symbol,
            signature: item
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
        });
    }
    hits
}

/// Sin ast-grep instalado: busqueda textual. Se dice explicitamente que es
/// texto y no estructura, para que el agente no se fie del formato.
fn text_fallback(
    dir: &Path,
    query: &str,
    kind: Option<&str>,
    limit: usize,
) -> String {
    let keywords = [
        "fn", "func", "def", "class", "struct", "enum", "trait", "interface", "fun",
        "impl", "const",
    ];
    let name = regex::escape(query);
    let alts = keywords.join("|");
    let Ok(re) = regex::Regex::new(&format!(r"(?i)\b(?:{alts})\s+{name}\b")) else {
        return "ast-grep not installed and the fallback pattern failed to build".to_string();
    };
    let mut lines = Vec::new();
    let _ = grep_walk_dir(dir, &re, limit, &mut lines);
    let header = format!(
        "NOTE: ast-grep is not installed, so this is a TEXT search, not a \
structural one. Results may include comments and call sites. Install \
ast-grep for reliable results.\n"
    );
    if let Some(k) = kind {
        return format!(
            "{header}# kind='{k}' cannot be honoured without ast-grep\n{}",
            lines.join("\n")
        );
    }
    if lines.is_empty() {
        return format!("{header}no definition of '{query}' found by text search");
    }
    format!("{header}{}", lines.join("\n"))
}
#[cfg(test)]
mod tests {
    use super::*;

    struct AnySandbox;

    impl SandboxConfig for AnySandbox {
        fn enabled(&self) -> bool {
            true
        }
        fn is_writable(&self, _: &Path) -> bool {
            false
        }
        fn is_readable(&self, _: &Path) -> bool {
            true
        }
    }

    fn tool_for(dir: &Path) -> SymbolsTool {
        SymbolsTool {
            workspace_root: dir.to_path_buf(),
            sandbox: Arc::new(RwLock::new(Box::new(AnySandbox))),
        }
    }

    async fn run(dir: &Path, query: &str, kind: Option<&str>) -> String {
        let mut args = serde_json::json!({ "query": query, "path": "." });
        if let Some(k) = kind {
            args["kind"] = serde_json::json!(k);
        }
        tool_for(dir)
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                args,
            )
            .await
            .expect("execute")
    }

    /// El test que mantiene viva la tabla: un fichero por lenguaje con un
    /// simbolo conocido, y se comprueba que la tool lo encuentra.
    ///
    /// Si alguien anade un patron que no casa con el binario, o cambia una
    /// extension, esto falla. Es la red que hacia falta despues de varios
    /// "arreglar el regex" sin cobertura real.
    #[tokio::test]
    async fn la_tabla_encuentra_lo_que_declara() {
        // (lenguaje, extension, fuente, tipo esperado)
        let cases: &[(&str, &str, &str, &str)] = &[
            ("python", "py", "def objetivo(datos):\n    return datos\n", "function"),
            ("python", "py", "class Objetivo:\n    pass\n", "class"),
            ("python", "py", "class Hija(Base):\n    pass\n", "class"),
            ("rust", "rs", "fn objetivo(x: u32) -> u32 { x }\n", "function"),
            ("rust", "rs", "fn objetivo() {}\n", "function"),
            ("rust", "rs", "async fn objetivo() -> u32 { 1 }\n", "function"),
            ("rust", "rs", "async fn objetivo() {}\n", "function"),
            ("rust", "rs", "struct Objetivo { a: u32 }\n", "type"),
            ("rust", "rs", "impl Objetivo { fn f(&self) {} }\n", "impl"),
            ("javascript", "js", "function objetivo(a, b) { return a; }\n", "function"),
            ("javascript", "js", "class Objetivo { m() {} }\n", "class"),
            ("javascript", "js", "const objetivo = 42;\n", "value"),
            ("typescript", "ts", "function objetivo(a: number): number { return a; }\n", "function"),
            ("typescript", "ts", "function objetivo(a) { return a; }\n", "function"),
            ("typescript", "ts", "interface Objetivo { a: number; }\n", "type"),
            ("go", "go", "package m\nfunc Objetivo(a string) string { return a }\n", "function"),
            ("go", "go", "package m\nfunc Objetivo() {}\n", "function"),
            ("go", "go", "package m\ntype Objetivo struct { A int }\n", "type"),
            ("java", "java", "class Objetivo { }\n", "class"),
            ("java", "java", "class C { int objetivo(int a) { return a; } }\n", "function"),
            ("java", "java", "class C { void objetivo(int a) {} }\n", "function"),
            ("ruby", "rb", "def objetivo(a)\n  a\nend\n", "function"),
            ("ruby", "rb", "class Objetivo\nend\n", "class"),
            ("php", "php", "<?php\nfunction objetivo($a) { return $a; }\n", "function"),
            ("php", "php", "<?php\nfunction objetivo(int $a): int { return $a; }\n", "function"),
            ("php", "php", "<?php\nclass Objetivo {}\n", "class"),
            ("csharp", "cs", "class Objetivo { }\n", "class"),
            ("cpp", "cpp", "int objetivo(int a) { return a; }\n", "function"),
            ("c", "c", "int objetivo(int a) { return a; }\n", "function"),
            ("kotlin", "kt", "fun objetivo(a: Int): Int { return a }\n", "function"),
            ("kotlin", "kt", "fun objetivo(a: Int) {}\n", "function"),
        ];

        let Some(bin) = find_ast_grep().await else {
            // Sin ast-grep no hay nada que verificar: la tabla es para el.
            eprintln!("ast-grep no instalado; se omite la verificacion de la tabla");
            return;
        };
        let _ = &bin;

        let mut fallos: Vec<String> = Vec::new();
        for (lang, ext, src, kind) in cases {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join(format!("m.{ext}"));
            std::fs::write(&file, src).unwrap();
            let out = run(dir.path(), "objetivo", Some(kind)).await;
            if !out.contains("objetivo") || out.starts_with('#') {
                fallos.push(format!(
                    "{lang} ({ext}, {kind}): no encontrado\n    fuente: {:?}\n    salida: {}",
                    src.trim(),
                    out.lines().next().unwrap_or("")
                ));
            }
        }
        assert!(
            fallos.is_empty(),
            "estos patrones de la tabla no encuentran lo que declaran:\n{}",
            fallos.join("\n")
        );
    }

    /// El agente no debe poder volver a meter un patron: se rechaza con un
    /// mensaje que dice que hacer, en vez de adivinar.
    #[tokio::test]
    async fn rechaza_patrones_en_crudo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.py"), "def f():\n    pass\n").unwrap();
        let err = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({ "query": "def $FUNC($$$ARGS):" }),
            )
            .await
            .expect_err("debe rechazar el patron");
        assert!(err.contains("symbol NAME"), "{err}");
    }

    #[tokio::test]
    async fn rechaza_kind_invalido() {
        let dir = tempfile::tempdir().unwrap();
        let err = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({ "query": "x", "kind": "inventado" }),
            )
            .await
            .expect_err("debe rechazar el kind");
        assert!(err.contains("function"), "{err}");
    }

    #[tokio::test]
    async fn query_vacio_o_con_espacios_es_error() {
        let dir = tempfile::tempdir().unwrap();
        for q in ["", "   "] {
            assert!(tool_for(dir.path())
                .execute(
                    &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                    serde_json::json!({ "query": q }),
                )
                .await
                .is_err());
        }
        let err = tool_for(dir.path())
            .execute(
                &crate::ExecuteContext { agent_id: "t".into(), cancel: None, http_client: None },
                serde_json::json!({ "query": "mi funcion" }),
            )
            .await
            .expect_err("los espacios son un error claro");
        assert!(err.contains("single symbol"), "{err}");
    }

    /// Busca en un arbol mixto sin que el llamante diga que hay dentro.
    #[tokio::test]
    async fn detecta_lenguajes_en_un_arbol_mixto() {
        if find_ast_grep().await.is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn comun(x: u32) -> u32 { x }\n").unwrap();
        std::fs::write(
            dir.path().join("b.py"),
            "def comun(datos):\n    return datos\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("c.go"),
            "package m\nfunc comun(x int) int { return x }\n",
        )
        .unwrap();

        let out = run(dir.path(), "comun", Some("function")).await;
        // El nombre es el mismo en los tres, cada uno con su idioma.
        assert!(out.contains("rust |"), "{out}");
        assert!(out.contains("python |"), "{out}");
        assert!(out.contains("go |"), "{out}");
    }

    /// Lo que se busca es la DEFINICION, no las llamadas ni los comentarios.
    #[tokio::test]
    async fn no_devuelve_llamadas_ni_comentarios() {
        if find_ast_grep().await.is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.py"),
            "# funcion_sh no esta definida aqui\n\
             def otra():\n    funcion_sh(1)\n    return 2\n\
             def funcion_sh(x):\n    return x\n",
        )
        .unwrap();
        let out = run(dir.path(), "funcion_sh", Some("function")).await;
        let hits = out.lines().filter(|l| l.contains("funcion_sh")).count();
        assert_eq!(hits, 1, "solo la definicion, no el comentario ni la llamada:\n{out}");
    }

    /// Sin coincidencia exacta debe decirlo claro, no fingir que no hay nada
    /// que buscar. Este era el modo de fallo silencioso que motivo el rediseno.
    #[tokio::test]
    async fn sin_coincidencia_explica_que_se_busco() {
        if find_ast_grep().await.is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.py"), "def f():\n    pass\n").unwrap();
        let out = run(dir.path(), "no_existe_este_simbolo", None).await;
        assert!(out.contains("no definition named"), "{out}");
        assert!(out.contains("languages:"), "debe decir que lenguajes miro: {out}");
    }

    /// Si solo hay coincidencias parciales, se avisa: el agente tiene que saber
    /// que no busco lo que pedia exactamente.
    #[tokio::test]
    async fn avisa_cuando_solo_encuentra_coincidencias_parciales() {
        if find_ast_grep().await.is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.py"),
            "def procesar_datos():\n    pass\n",
        )
        .unwrap();
        let out = run(dir.path(), "procesar", Some("function")).await;
        assert!(out.contains("CONTAIN it"), "{out}");
        assert!(out.contains("procesar_datos"), "{out}");
    }

    /// Un arbol sin codigo fuente debe decirlo, no devolver un error raro.
    #[tokio::test]
    async fn arbol_sin_codigo_fuente() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notas.txt"), "hola").unwrap();
        let out = run(dir.path(), "cualquiera", None).await;
        assert!(out.contains("no source files"), "{out}");
    }

    /// `path` no puede salirse del workspace.
    ///
    /// Este test existe porque `docs/audit/SUMMARY.md` afirmaba que symbols
    /// usaba `resolve_under_workspace` y nunca lo uso: hacia
    /// `workspace_root.join(raw)` y ya. `join` con una ruta absoluta
    /// DESCARTA la base, asi que `path: "/etc"` se aceptaba entero, y un
    /// `../` subia hasta el directorio padre del workspace. La tool no
    /// devuelve el cuerpo del fichero, pero si las rutas y la primera linea
    /// de cada definicion que encuentre fuera del workspace.
    ///
    /// El resto de tools de filesystem (`read_file`, `write_file`, `glob`,
    /// `grep`, `list_dir`) si pasan por el helper; faltaba aqui.
    #[tokio::test]
    async fn no_sale_del_workspace_por_path() {
        let ws = tempfile::tempdir().unwrap();
        let fuera = ws.path().parent().unwrap().join(format!(
            "neurox-symbols-fuera-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&fuera).unwrap();
        std::fs::write(
            fuera.join("secreto.py"),
            "def objetivo_secreto():\n    return 1\n",
        )
        .unwrap();

        let err = tool_for(ws.path())
            .execute(
                &crate::ExecuteContext {
                    agent_id: "t".into(),
                    cancel: None,
                    http_client: None,
                },
                serde_json::json!({ "query": "objetivo_secreto", "path": format!("../neurox-symbols-fuera-{}", std::process::id()) }),
            )
            .await
            .expect_err("un path fuera del workspace debe rechazarse");

        let _ = std::fs::remove_dir_all(&fuera);
        assert!(err.contains("outside workspace"), "{err}");
    }

    /// Una ruta ABSOLUTA tambien se rechaza, que es el caso que `join` mas
    /// facil de dejar pasar: `Path::join` con un absoluto ignora la base.
    #[tokio::test]
    async fn rechaza_path_absoluta_fuera_del_workspace() {
        let ws = tempfile::tempdir().unwrap();
        let err = tool_for(ws.path())
            .execute(
                &crate::ExecuteContext {
                    agent_id: "t".into(),
                    cancel: None,
                    http_client: None,
                },
                serde_json::json!({ "query": "cualquiera", "path": "/etc" }),
            )
            .await
            .expect_err("una ruta absoluta fuera del workspace debe rechazarse");
        assert!(err.contains("outside workspace"), "{err}");
    }

    /// Y lo de dentro sigue funcionando: el helper no puede romper el caso
    /// normal ni un subdirectorio legitimo.
    #[tokio::test]
    async fn sigue_dejando_buscar_dentro_del_workspace() {
        let ws = tempfile::tempdir().unwrap();
        let sub = ws.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.py"), "def comun():\n    return 1\n").unwrap();

        let raiz = tool_for(ws.path())
            .execute(
                &crate::ExecuteContext {
                    agent_id: "t".into(),
                    cancel: None,
                    http_client: None,
                },
                serde_json::json!({ "query": "comun", "path": "sub" }),
            )
            .await
            .expect("un subdirectorio del workspace es valido");
        assert!(raiz.contains("comun"), "{raiz}");
    }

    /// Un symlink DENTRO del workspace que apunte fuera tampoco vale.
    ///
    /// El chequeo lexico de `resolve_under_workspace` no ve symlinks: la ruta
    /// es legxima y esta dentro, luego pasa, y el recorrido seguia el enlace
    /// al directorio de al lado. Por eso esta el segundo chequeo, sobre la
    /// ruta ya canonicalizada.
    #[cfg(unix)]
    #[tokio::test]
    async fn no_sigue_un_symlink_que_apunta_fuera() {
        let ws = tempfile::tempdir().unwrap();
        let fuera = tempfile::tempdir().unwrap();
        std::fs::write(
            fuera.path().join("secreto.py"),
            "def objetivo_secreto():\n    return 1\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(fuera.path(), ws.path().join("enlace")).unwrap();

        let err = tool_for(ws.path())
            .execute(
                &crate::ExecuteContext {
                    agent_id: "t".into(),
                    cancel: None,
                    http_client: None,
                },
                serde_json::json!({ "query": "objetivo_secreto", "path": "enlace" }),
            )
            .await
            .expect_err("un symlink hacia fuera no debe seguirse");
        assert!(err.contains("outside workspace"), "{err}");
    }

    /// El filtro `kind` tiene que excluir de verdad lo que no sea de ese tipo.
    #[tokio::test]
    async fn el_filtro_kind_excluye() {
        if find_ast_grep().await.is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.py"),
            "class misma():\n    pass\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.rs"),
            "fn misma() {}\n",
        )
        .unwrap();
        let out = run(dir.path(), "misma", Some("function")).await;
        assert!(out.contains("rust |"), "{out}");
        assert!(!out.contains("python |"), "la clase no es una function:\n{out}");
    }
}
