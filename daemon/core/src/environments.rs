//! Env management — persist variables de entorno del daemon (EP-0017-04).
//!
//! Endpoint dedicado para gestionar variables de entorno de forma persistente:
//! escribe en el archivo env del daemon (ubicación configurable via
//! `NEUROX_ENV_FILE`, default `~/.config/neurox/env` — el
//! `EnvironmentFile` del unit systemd) y las propaga al proceso actual con
//! `std::env::set_var`.
//!
//! Reglas:
//! - Solo nombres `UPPER_SNAKE_CASE` (`^[A-Z][A-Z0-9_]*$`).
//! - Los valores NUNCA se devuelven por API (solo estado `set`).
//! - Escritura atómica (temp + rename) con permisos 0600.
//! - Los tests siempre apuntan `NEUROX_ENV_FILE` a un temp dir y limpian
//!   con `std::env::remove_var`.

use std::path::{Path, PathBuf};

/// Default env file path when `NEUROX_ENV_FILE` is not set.
/// Matches the daemon's systemd unit: `EnvironmentFile=%h/.config/neurox/env`.
pub fn default_env_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neurox")
        .join("env")
}

/// Resolve the env file path from `NEUROX_ENV_FILE` or the default.
pub fn env_file_path() -> PathBuf {
    match std::env::var("NEUROX_ENV_FILE") {
        Ok(p) if !p.is_empty() => {
            let expanded = if let Some(stripped) = p.strip_prefix("~/") {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(stripped)
            } else {
                PathBuf::from(p)
            };
            expanded
        }
        _ => default_env_file_path(),
    }
}

/// Validate an env var name is `UPPER_SNAKE_CASE` (`^[A-Z][A-Z0-9_]*$`).
pub fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("env var name must not be empty".into());
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_uppercase() {
        return Err(format!(
            "invalid env var name '{key}' — must be UPPER_SNAKE_CASE (e.g. MINIMAX_API_KEY)"
        ));
    }
    for c in chars {
        if !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            return Err(format!(
                "invalid env var name '{key}' — must be UPPER_SNAKE_CASE (e.g. MINIMAX_API_KEY)"
            ));
        }
    }
    Ok(())
}

/// Parse an env file into lines preserving comments/order.
/// Returns `(lines, existing_value)` where `existing_value` is `Some` if the
/// key already appears (last occurrence wins, per dotenv semantics).
fn parse_env_file(content: &str, key: &str) -> (Vec<String>, Option<String>) {
    let mut lines: Vec<String> = Vec::new();
    let mut existing: Option<String> = None;
    let prefix = format!("{key}=");
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(&prefix) {
            existing = Some(rest.to_string());
            // Drop the old line; it will be re-appended by the writer.
            continue;
        }
        lines.push(line.to_string());
    }
    (lines, existing)
}

/// Merge a `KEY=value` into the env file atomically (temp + rename) with
/// 0600 permissions. Preserves comments, order, and other vars.
pub fn merge_env_file(path: &Path, key: &str, value: &str) -> Result<(), String> {
    validate_key(key)?;
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("failed to read env file {}: {e}", path.display())),
    };
    let (mut lines, _existing) = parse_env_file(&content, key);
    lines.push(format!("{key}={value}"));

    // Ensure parent dir exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create env dir {}: {e}", parent.display()))?;
    }

    // Atomic write: temp file in the same dir + rename.
    let tmp = path.with_extension("tmp");
    write_with_0600(&tmp, &lines.join("\n"))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("failed to rename env file {}: {e}", path.display()))?;
    Ok(())
}

/// Write file content with 0600 permissions (secrets). Uses `OpenOptions`
/// with mode on unix; falls back to plain write elsewhere.
fn write_with_0600(path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true).mode(0o600);
        let mut f = opts
            .open(path)
            .map_err(|e| format!("failed to open {}: {e}", path.display()))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        f.sync_all()
            .map_err(|e| format!("failed to sync {}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Set an env var: persist to file + apply to the current process.
pub fn set_env_var(key: &str, value: &str) -> Result<(), String> {
    validate_key(key)?;
    let path = env_file_path();
    merge_env_file(&path, key, value)?;
    std::env::set_var(key, value);
    Ok(())
}

/// Unset an env var: drop the key from the env file (atomic write,
/// 0600) and remove it from the current process. Returns `Ok(())`
/// when the key wasn't present — idempotent, so a double DELETE from
/// the frontend doesn't 500. `validate_key` still applies (an invalid
/// name never made it into the file in the first place).
pub fn unset_env_var(key: &str) -> Result<(), String> {
    validate_key(key)?;
    let path = env_file_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::env::remove_var(key);
            return Ok(());
        }
        Err(e) => return Err(format!("failed to read env file {}: {e}", path.display())),
    };

    let prefix = format!("{key}=");
    let mut kept: Vec<String> = Vec::new();
    let mut found = false;
    for line in content.lines() {
        if line.starts_with(&prefix) {
            found = true;
            continue;
        }
        kept.push(line.to_string());
    }

    // Ensure parent dir exists (the file might not be there yet on a
    // fresh install; nothing to write, but `unset` still succeeds).
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create env dir {}: {e}", parent.display()))?;
    }

    let tmp = path.with_extension("tmp");
    write_with_0600(&tmp, &kept.join("\n"))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("failed to rename env file {}: {e}", path.display()))?;

    std::env::remove_var(key);
    tracing::debug!(key = %key, removed = found, "env var unset");
    Ok(())
}

/// List env var names from the env file (values never included).
pub fn list_env_file_keys() -> Result<Vec<String>, String> {
    let path = env_file_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("failed to read env file {}: {e}", path.display())),
    };
    let mut keys: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let k = trimmed[..eq].trim();
            if !k.is_empty() && validate_key(k).is_ok() {
                keys.push(k.to_string());
            }
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Global lock serializes tests that touch `NEUROX_ENV_FILE` and
    /// temp dirs — cargo runs tests in parallel by default.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn unique_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("np-env-{tag}-{}", std::process::id()))
    }

    #[test]
    fn validate_key_accepts_upper_snake() {
        assert!(validate_key("A").is_ok());
        assert!(validate_key("MINIMAX_API_KEY").is_ok());
        assert!(validate_key("ANTHROPIC_API_KEY_2").is_ok());
    }

    #[test]
    fn validate_key_rejects_bad_names() {
        assert!(validate_key("a").is_err());
        assert!(validate_key("1A").is_err());
        assert!(validate_key("PATH!").is_err());
        assert!(validate_key("con-espacio").is_err());
        assert!(validate_key("").is_err());
    }

    #[test]
    fn env_file_path_defaults_to_config_dir() {
        let _g = lock();
        std::env::remove_var("NEUROX_ENV_FILE");
        let p = env_file_path();
        assert!(p.ends_with("neurox/env"));
    }

    #[test]
    fn env_file_path_expands_tilde() {
        let _g = lock();
        std::env::set_var("NEUROX_ENV_FILE", "~/tmp/np-env-test");
        let p = env_file_path();
        std::env::remove_var("NEUROX_ENV_FILE");
        let home = dirs::home_dir().expect("home dir");
        assert_eq!(p, home.join("tmp/np-env-test"));
    }

    #[test]
    fn merge_creates_new_file() {
        let _g = lock();
        let dir = unique_dir("create");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("env");
        let _ = std::fs::remove_file(&path);

        merge_env_file(&path, "MINIMAX_API_KEY", "sk-test").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("MINIMAX_API_KEY=sk-test"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn merge_replaces_existing_key_preserving_rest() {
        let _g = lock();
        let dir = unique_dir("replace");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("env");
        std::fs::write(&path, "# comment\nMINIMAX_API_KEY=old\nOTHER=keep\n").unwrap();

        merge_env_file(&path, "MINIMAX_API_KEY", "new").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# comment"));
        assert!(content.contains("OTHER=keep"));
        assert!(content.contains("MINIMAX_API_KEY=new"));
        assert!(!content.contains("MINIMAX_API_KEY=old"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn merge_permits_are_0600() {
        #[cfg(unix)]
        {
            let _g = lock();
            use std::os::unix::fs::PermissionsExt;
            let dir = unique_dir("perm");
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("env");
            let _ = std::fs::remove_file(&path);

            merge_env_file(&path, "TEST_KEY", "v").unwrap();
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    #[test]
    fn set_env_var_persists_and_applies() {
        let _g = lock();
        let dir = unique_dir("setvar");
        std::env::set_var("NEUROX_ENV_FILE", dir.join("env"));
        let result = set_env_var("NP_TEST_KEY", "hello-world");
        std::env::remove_var("NEUROX_ENV_FILE");
        assert!(result.is_ok());
        assert_eq!(
            std::env::var("NP_TEST_KEY").ok().as_deref(),
            Some("hello-world")
        );
        std::env::remove_var("NP_TEST_KEY");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_env_file_keys_skips_comments_and_invalid() {
        let _g = lock();
        let dir = unique_dir("listkeys");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("env");
        std::fs::write(
            &path,
            "# comment\nMINIMAX_API_KEY=sk\nlower=skip\nOTHER=keep\n",
        )
        .unwrap();
        std::env::set_var("NEUROX_ENV_FILE", path.to_str().unwrap());
        let keys = list_env_file_keys().unwrap();
        std::env::remove_var("NEUROX_ENV_FILE");
        std::fs::remove_dir_all(&dir).ok();
        assert!(keys.contains(&"MINIMAX_API_KEY".to_string()));
        assert!(keys.contains(&"OTHER".to_string()));
        assert!(!keys.contains(&"lower".to_string()));
    }
}
