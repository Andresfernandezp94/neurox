//! Local GGUF model discovery (EP-0018-05).
//!
//! Reads `NEUROX_MODELS_DIR` (default `~/models`) and lists GGUF files
//! under that directory, RECURSIVAMENTE.
//!
//! ## Categorias por carpeta
//!
//! El subdirectorio inmediato de la raiz es la categoria del modelo:
//! `~/tools/models/chat/qwen.gguf` es `chat`,
//! `~/tools/models/embedding/bge.gguf` es `embedding`.
//!
//! La organizacion en carpetas ya existe en el disco de los operadores; el
//! listado era plano y solo mostraba la primera capa, asi que con
//! `chat/` y `embedding/` el operador via 3 de sus 7 modelos y ningun
//! modelo de chat. La carpeta es la categoria porque es lo que el operador
//! ya expresa con la disposicion del disco: no hay que duplicarlo en metadata ni
//! inferirlo del nombre del archivo (que es menos fiable: `bge-m3` no dice
//! "embedding", hay que saberlo).
//!
//! Un `.gguf` en la raiz (sin carpeta) es categoria `unclassified`.
//! En un arbol anidado gana el subdirectorio mas cercano al archivo:
//! `models/llm/chat/x.gguf` es `chat`, no `llm`. El nivel superior
//! agrupa; el ultimo decide.
//!
//! El daemon usa esto para:
//!
//! - `GET /v1/llm/models/local` — surfaces available local models so the
//!   selector is a separate concern).
//! - Early validation in the orchestrator: when starting a local service,
//!   we check that `local_model_path` exists before spawning the child,
//!   so the operator gets a clear `last_error` instead of a confusing
//!   llama-server crash.
//!
//! Tilde (`~`) is expanded to the user's home directory.

use std::path::{Path, PathBuf};

/// Environment variable that points at the directory the daemon scans for
/// GGUF models. Default: `~/models`.
pub const MODELS_DIR_ENV: &str = "NEUROX_MODELS_DIR";

/// Default models directory (relative to home).
pub const DEFAULT_MODELS_DIR: &str = "~/models";

/// Return the resolved models directory, expanding `~` and reading the
/// env var. Returns `None` if the directory does not exist.
pub fn models_dir() -> Option<PathBuf> {
    let raw = std::env::var(MODELS_DIR_ENV)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_MODELS_DIR.to_string());
    let expanded = expand_tilde(Path::new(&raw));
    if expanded.is_dir() {
        Some(expanded)
    } else {
        None
    }
}

/// Lightweight descriptor of a GGUF file in the models directory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalModel {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
    /// Categoria del modelo: el subdirectorio inmediato bajo la raiz de
    /// `NEUROX_MODELS_DIR`. `None` cuando el `.gguf` esta en la raiz misma.
    pub category: Option<String>,
}

impl LocalModel {
    /// Categoria efectiva, normalizada para la UI. Un `.gguf` suelto en la
    /// raiz no tiene carpeta que lo clasifique, y el operador tiene que
    /// poder distinguir "no lo categorize" de un categoria de verdad.
    pub fn category_or_unclassified(&self) -> &str {
        self.category.as_deref().unwrap_or(UNCLASSIFIED)
    }
}

/// Categoria de un `.gguf` que esta en la raiz, sin carpeta que lo
/// clasifique. No es una categoria real: es el estado "el operador todavia
/// no lo organizo".
pub const UNCLASSIFIED: &str = "unclassified";

/// Scan the models directory for `*.gguf` files, RECURSIVELY.
///
/// Antes era plano: `read_dir` sobre la raiz y `if !path.is_file()
/// { continue }` saltaba los directorios, asi que con `chat/` y
/// `embedding/` el operador via 3 de sus 7 modelos y ningun modelo de
/// chat. La carpeta es la categoria (ver la doc del modulo).
///
/// La recursion tiene un tope de profundidad: un arbol de symlinks mal
/// armado puede ciclar y `read_dir` no lo detecta. Con `MAX_DEPTH` el peor
/// caso es acotado en vez de colgarse.
pub fn list_local_gguf() -> Vec<LocalModel> {
    let Some(dir) = models_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_gguf(&dir, None, 0, &mut out);
    // Orden estable: por categoria y despues por filename, para que el
    // operador vea los grupos siempre en el mismo orden.
    out.sort_by(|a, b| {
        let ca = a.category_or_unclassified();
        let cb = b.category_or_unclassified();
        ca.cmp(cb)
            .then_with(|| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()))
    });
    out
}

/// Profundidad maxima de recursion. `chat/` y `embedding/` estan en 1; el
/// tope existe solo para acotar el peor caso.
const MAX_DEPTH: usize = 4;

fn collect_gguf(dir: &Path, category: Option<&str>, depth: usize, out: &mut Vec<LocalModel>) {
    if depth > MAX_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // El subdirectorio MAS CERCANO al archivo define la categoria:
            // `llm/chat/x.gguf` es `chat`, no `llm`. Por eso el nombre del
            // subdirectorio actual pisa al heredado.
            //
            // Ojo: `category.or(...)` daria el ancestro y seria al reves:
            // con `llm/chat/`, `llm` ganaria por ser el primer nivel visto,
            // y el operador veria toda la carpeta de chat bajo `llm`.
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            collect_gguf(&path, Some(name), depth + 1, out);
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("gguf") {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let filename = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        out.push(LocalModel {
            filename,
            path: path.to_string_lossy().to_string(),
            size_bytes: meta.len(),
            category: category.map(str::to_string),
        });
    }
}

/// Expand a leading `~/` to the user's home directory. Bare `~` is left
/// alone (returns as-is) so the caller can decide what to do.
fn expand_tilde(path: &Path) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if s == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Global lock serializes tests that touch `NEUROX_MODELS_DIR`.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn unique_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("np-models-{tag}-{}", std::process::id()))
    }

    #[test]
    fn models_dir_uses_default_when_env_unset() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(MODELS_DIR_ENV);
        // Default `~/models` may or may not exist on this host; we only
        // assert that the helper respects the env-or-default contract.
        let dir = models_dir();
        if let Some(d) = dir {
            assert!(d.ends_with("models"));
        }
    }

    #[test]
    fn models_dir_expands_tilde() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var(MODELS_DIR_ENV, "~/tmp/np-models-tilde-test");
        let dir = models_dir();
        std::env::remove_var(MODELS_DIR_ENV);
        let home = dirs::home_dir().expect("home dir");
        // May or may not exist — we just check the expansion.
        if let Some(d) = dir {
            assert!(d.starts_with(home));
        }
    }

    #[test]
    fn models_dir_respects_env_var() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("exists");
        std::fs::create_dir_all(&dir).unwrap();
        let dir_for_assert = dir.clone();
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let resolved = models_dir();
        std::env::remove_var(MODELS_DIR_ENV);
        assert_eq!(resolved, Some(dir_for_assert));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn models_dir_returns_none_for_missing_path() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("missing");
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let resolved = models_dir();
        std::env::remove_var(MODELS_DIR_ENV);
        assert_eq!(resolved, None);
    }

    #[test]
    fn list_local_gguf_returns_only_gguf_files() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("list");
        std::fs::create_dir_all(&dir).unwrap();
        // One GGUF + one non-GGUF + one subdir.
        std::fs::write(dir.join("model-a.gguf"), b"a").unwrap();
        std::fs::write(dir.join("README.md"), b"r").unwrap();
        std::fs::create_dir(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/model-b.gguf"), b"b").unwrap();
        // Non-default extension uppercase.
        std::fs::write(dir.join("model-c.GGUF"), b"c").unwrap();
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        let names: Vec<&str> = models.iter().map(|m| m.filename.as_str()).collect();
        assert!(names.contains(&"model-a.gguf"));
        assert!(
            names.contains(&"model-c.GGUF"),
            "uppercase GGUF should match"
        );
        assert!(!names.contains(&"README.md"));
        // Los subdirectorios ya NO se saltan: la carpeta es la categoria
        // del modelo. Antes el assert afirmaba lo contrario y fijaba el
        // comportamiento plano.
        assert!(
            names.contains(&"model-b.gguf"),
            "gguf inside a category subdir must be listed"
        );
        assert_eq!(models.len(), 3);
        // Y `nested/model-b` conserva la carpeta como categoria.
        let b = models
            .iter()
            .find(|m| m.filename == "model-b.gguf")
            .unwrap();
        assert_eq!(b.category.as_deref(), Some("nested"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// El caso real que motivo la recursion: con `chat/` y `embedding/`, el
    /// listado plano mostraba 3 de 7 modelos y ningun de chat.
    #[test]
    fn list_local_gguf_groups_by_subdirectory() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("categories");
        for sub in ["chat", "embedding"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
            std::fs::write(dir.join(sub).join("m.gguf"), b"x").unwrap();
        }
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(models.len(), 2);
        let mut cats: Vec<&str> = models
            .iter()
            .map(|m| m.category_or_unclassified())
            .collect();
        cats.sort_unstable();
        assert_eq!(cats, vec!["chat", "embedding"]);
    }

    #[test]
    fn gguf_at_the_root_is_unclassified() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("unclassified");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("suelto.gguf"), b"x").unwrap();
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(models.len(), 1);
        // Sin carpeta no hay categoria, y el operador tiene que poder
        // distinguir "no lo organize" de una categoria real.
        assert_eq!(models[0].category, None);
        assert_eq!(models[0].category_or_unclassified(), UNCLASSIFIED);
    }

    #[test]
    fn nested_subdirs_inherit_the_nearest_category() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // `llm/chat/x.gguf`: la categoria es `chat`, no `llm`. Gana el
        // subdirectorio mas cercano al archivo, no el primer nivel visto:
        // el nivel superior agrupa, el ultimo decide.
        let dir = unique_dir("nested-cat");
        std::fs::create_dir_all(dir.join("llm/chat")).unwrap();
        std::fs::write(dir.join("llm/chat/x.gguf"), b"x").unwrap();
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].category.as_deref(), Some("chat"));
    }

    #[test]
    fn results_are_sorted_by_category_then_filename() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("sort");
        for (sub, f) in [("zebra", "b.gguf"), ("chat", "z.gguf"), ("chat", "a.gguf")] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
            std::fs::write(dir.join(sub).join(f), b"x").unwrap();
        }
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        std::fs::remove_dir_all(&dir).ok();

        let order: Vec<(&str, &str)> = models
            .iter()
            .map(|m| (m.category_or_unclassified(), m.filename.as_str()))
            .collect();
        assert_eq!(
            order,
            vec![("chat", "a.gguf"), ("chat", "z.gguf"), ("zebra", "b.gguf")],
            "groups must be stable so the UI does not reshuffle"
        );
    }

    #[test]
    fn recursion_stops_at_max_depth() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Un arbol profundo no debe colgarse: el tope acota el peor caso
        // en vez de confiar en que no haya symlinks ciclicos.
        let dir = unique_dir("depth");
        let mut p = dir.clone();
        for i in 0..MAX_DEPTH + 3 {
            p = p.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("deep.gguf"), b"x").unwrap();
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        std::fs::remove_dir_all(&dir).ok();

        assert!(
            models.is_empty(),
            "beyond MAX_DEPTH the model is out of reach by design"
        );
    }

    #[test]
    fn list_local_gguf_returns_empty_when_dir_missing() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir("missing-list");
        std::env::set_var(MODELS_DIR_ENV, dir.to_str().unwrap());
        let models = list_local_gguf();
        std::env::remove_var(MODELS_DIR_ENV);
        assert!(models.is_empty());
    }
}
