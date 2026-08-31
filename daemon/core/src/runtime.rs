use std::path::PathBuf;

#[must_use]
pub fn data_dir() -> PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    p
}

#[must_use]
pub fn config_dir() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    std::fs::create_dir_all(&p).ok();
    p
}

#[must_use]
pub fn cache_dir() -> PathBuf {
    let mut p = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    p
}

#[must_use]
pub fn default_db_path() -> PathBuf {
    let mut p = data_dir();
    p.push("neurox.db");
    p
}

#[must_use]
pub fn default_config_path() -> PathBuf {
    let mut p = config_dir();
    p.push("config.yaml");
    p
}

pub fn default_socket_dir() -> PathBuf {
    let runtime_dir =
        std::env::var_os("XDG_RUNTIME_DIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    let mut p = runtime_dir;
    p.push("neurox");
    p
}

/// Directory where each component (memoryd, voiced, clickup, …) writes
/// one JSONL file per process under `<component>.jsonl`. Exposed via
/// `GET /v1/logs?component=…&level=…&limit=…&since=…`.
#[must_use]
pub fn default_logs_dir() -> PathBuf {
    let mut p = data_dir();
    p.push("logs");
    p
}

pub fn init_dirs() -> anyhow::Result<()> {
    for dir in &[data_dir(), config_dir(), default_socket_dir(), default_logs_dir()] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
