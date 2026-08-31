//! Public surface of the model-configs store.
//!
//! Mirrors the `LlmProviderStore` pattern: a thin wrapper around
//! `model_configs::{get, upsert, delete, list}` that holds the
//! `SqlitePool` and is injected into `AppState` as an `Arc`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::model_configs::{self, ModelConfig, StoredModelConfig};

/// Run the schema migration on the given pool. Idempotent.
pub async fn ensure_table(pool: &SqlitePool) -> Result<(), String> {
    model_configs::ensure_table(pool).await
}

/// Async-friendly handle around the model-configs SQLite table.
#[derive(Clone)]
pub struct ModelConfigStore {
    pool: Arc<SqlitePool>,
}

impl ModelConfigStore {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    /// Borrow the underlying pool handle. Used by `main.rs` to run the
    /// `ensure_table` migration before constructing the store.
    pub fn pool(&self) -> Arc<SqlitePool> {
        Arc::clone(&self.pool)
    }

    pub async fn get(&self, filename: &Path) -> Result<Option<StoredModelConfig>, String> {
        model_configs::get(&self.pool, filename).await
    }

    pub async fn upsert(
        &self,
        filename: &Path,
        config: &ModelConfig,
    ) -> Result<StoredModelConfig, String> {
        model_configs::upsert(&self.pool, filename, config).await
    }

    pub async fn delete(&self, filename: &Path) -> Result<(), String> {
        model_configs::delete(&self.pool, filename).await
    }

    pub async fn list(&self) -> Result<Vec<StoredModelConfig>, String> {
        model_configs::list(&self.pool).await
    }
}

/// Resolve a model filename from the URL path. The URL embeds the
/// absolute path of the GGUF, URL-encoded (`%2F` for separators).
/// We return `None` if the segment is empty.
pub fn parse_filename_path(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() {
        return None;
    }
    // Reject path traversal attempts before decoding.
    if raw.contains("..") {
        return None;
    }
    let decoded = percent_decode(raw);
    Some(PathBuf::from(decoded))
}

/// Tiny RFC-3986 percent decoder for path segments. Keeps `+`,
/// unreserved chars and `%XX` escapes; rejects malformed escapes.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
