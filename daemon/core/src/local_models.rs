//! Local GGUF model discovery (EP-0018-05).
//!
//! Reads `NEUROX_MODELS_DIR` (default `~/models`) and lists GGUF files
//! in that directory. The daemon uses this for:
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
}

/// Scan the models directory for `*.gguf` files and return their names +
/// absolute paths + sizes. Returns an empty Vec if the directory is
/// missing.
pub fn list_local_gguf() -> Vec<LocalModel> {
    let Some(dir) = models_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(it) => it,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        let path = entry.path();
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
        });
    }
    // Stable order: by filename (case-insensitive).
    out.sort_by(|a, b| {
        let ka = a.filename.to_lowercase();
        let kb = b.filename.to_lowercase();
        ka.cmp(&kb)
    });
    out
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
        assert!(
            !names.iter().any(|n| n.contains("nested/model-b")),
            "subdirs should be skipped"
        );
        assert_eq!(models.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
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
