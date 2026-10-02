//! SQLite persistence for per-model inference configuration.
//!
//! Each local GGUF model can carry a JSON-serialised config blob with
//! inference parameters (temperature, top_p, top_k, max_tokens, tokens
//! per second, stop sequences, etc.). The config is keyed by the
//! absolute path of the GGUF file so the same filename under different
//! `MODELS_DIR`s stays independent.
//!
//! Storage is intentionally simple: one row per model, single JSON
//! blob in the `config` column. We avoid a wide table because the
//! shape will grow as we add fields, and JSON is cheap to validate.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

/// Serializa un `Option<f32>` redondeando a 6 decimales.
///
/// Un f32 se guarda en SQLite como REAL y serde lo emite como f64 con la
/// expansion EXACTA: `0.3f32` sale `0.30000001192092896`. En la UI el campo
/// de temperature mostraba esa cadena entera y el operador no tenia forma de
/// saber que el numero que elia era 0.3.
///
/// 6 decimales es de sobra para sampling params (temperature y top_p se
/// mueven en pasos de centesimas) y deja de aparecer el ruido de coma
/// flotante. Solo afecta el JSON: el valor en la base sigue siendo el f32
/// exacto.
///
/// None se mantiene como None para no romper `skip_serializing_if`.
fn ser_f32_opt<S>(v: &Option<f32>, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match v {
        Some(x) => {
            let r = (*x as f64 * 1_000_000.0).round() / 1_000_000.0;
            // f64 → JSON. Si fuera NaN/inf, serde lo rechaza y el operador
            // veria un error de deserializacion; se cae a null en vez.
            if r.is_finite() {
                ser.serialize_f64(r)
            } else {
                ser.serialize_none()
            }
        }
        None => ser.serialize_none(),
    }
}

/// Inference parameters attached to a single local model. All fields
/// are optional so the operator can pick which to tune. Sensible
/// defaults are applied by the inference layer when a field is `None`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelConfig {
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "ser_f32_opt"
    )]
    pub temperature: Option<f32>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        serialize_with = "ser_f32_opt"
    )]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tokens_per_second: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub system_prompt: Option<String>,
}

/// Persisted row keyed by absolute path of the model file.
#[derive(Debug, Clone, Serialize)]
pub struct StoredModelConfig {
    pub filename: String,
    pub config: ModelConfig,
    pub updated_at: String,
}

/// Ensure the `model_configs` table exists (idempotent migration).
pub async fn ensure_table(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_configs (
            filename    TEXT PRIMARY KEY,
            config      TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("failed to create model_configs table: {e}"))?;
    Ok(())
}

/// Get the config for a model (keyed by absolute path).
pub async fn get(pool: &SqlitePool, filename: &Path) -> Result<Option<StoredModelConfig>, String> {
    let key = canonicalize_key(filename);
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT filename, config, updated_at FROM model_configs WHERE filename = ?")
            .bind(&key)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("failed to read model config: {e}"))?;
    let Some((filename, config_json, updated_at)) = row else {
        return Ok(None);
    };
    let config: ModelConfig = serde_json::from_str(&config_json)
        .map_err(|e| format!("malformed model config JSON: {e}"))?;
    Ok(Some(StoredModelConfig {
        filename,
        config,
        updated_at,
    }))
}

/// Insert or replace the config for a model. Returns the persisted row.
pub async fn upsert(
    pool: &SqlitePool,
    filename: &Path,
    config: &ModelConfig,
) -> Result<StoredModelConfig, String> {
    let key = canonicalize_key(filename);
    let now = chrono::Utc::now().to_rfc3339();
    let config_json = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialise model config: {e}"))?;
    sqlx::query(
        "INSERT INTO model_configs (filename, config, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(filename) DO UPDATE SET config = excluded.config, updated_at = excluded.updated_at",
    )
    .bind(&key)
    .bind(&config_json)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("failed to upsert model config: {e}"))?;
    Ok(StoredModelConfig {
        filename: key,
        config: config.clone(),
        updated_at: now,
    })
}

/// Delete the config for a model. No-op if absent.
pub async fn delete(pool: &SqlitePool, filename: &Path) -> Result<(), String> {
    let key = canonicalize_key(filename);
    sqlx::query("DELETE FROM model_configs WHERE filename = ?")
        .bind(&key)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to delete model config: {e}"))?;
    Ok(())
}

/// List every persisted model config, alphabetically by filename.
pub async fn list(pool: &SqlitePool) -> Result<Vec<StoredModelConfig>, String> {
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT filename, config, updated_at FROM model_configs ORDER BY filename")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("failed to list model configs: {e}"))?;
    rows.into_iter()
        .map(|(filename, config_json, updated_at)| {
            let config: ModelConfig = serde_json::from_str(&config_json)
                .map_err(|e| format!("malformed model config JSON: {e}"))?;
            Ok(StoredModelConfig {
                filename,
                config,
                updated_at,
            })
        })
        .collect()
}

/// Canonical key for the `filename` PK. We canonicalise the path so the
/// same physical file stored via different relative paths collapses
/// to a single row. If the file does not exist yet, fall back to the
/// raw input (operators can pre-create the row before downloading).
fn canonicalize_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_table(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn ensure_table_is_idempotent() {
        let pool = fresh_pool().await;
        // Call twice — must not fail.
        ensure_table(&pool).await.unwrap();
        ensure_table(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn upsert_and_get_roundtrip() {
        let pool = fresh_pool().await;
        let dir = std::env::temp_dir().join(format!("np-mcfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model.gguf");
        std::fs::write(&path, b"stub").unwrap();

        let cfg = ModelConfig {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(2048),
            tokens_per_second: Some(100),
            stop_sequences: Some(vec!["</s>".to_string()]),
            system_prompt: None,
        };
        upsert(&pool, &path, &cfg).await.unwrap();
        let got = get(&pool, &path).await.unwrap().unwrap();
        assert_eq!(got.config.temperature, Some(0.7));
        assert_eq!(got.config.top_p, Some(0.9));
        assert_eq!(
            got.config.stop_sequences.as_deref(),
            Some(&["</s>".to_string()][..])
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn upsert_overwrites_previous_config() {
        let pool = fresh_pool().await;
        let dir = std::env::temp_dir().join(format!("np-mcfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model.gguf");
        std::fs::write(&path, b"stub").unwrap();

        upsert(
            &pool,
            &path,
            &ModelConfig {
                temperature: Some(0.5),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        upsert(
            &pool,
            &path,
            &ModelConfig {
                temperature: Some(0.9),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let got = get(&pool, &path).await.unwrap().unwrap();
        assert_eq!(got.config.temperature, Some(0.9));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let pool = fresh_pool().await;
        let dir = std::env::temp_dir().join(format!("np-mcfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model.gguf");
        std::fs::write(&path, b"stub").unwrap();

        upsert(&pool, &path, &ModelConfig::default()).await.unwrap();
        assert!(get(&pool, &path).await.unwrap().is_some());
        delete(&pool, &path).await.unwrap();
        assert!(get(&pool, &path).await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn get_returns_none_when_missing() {
        let pool = fresh_pool().await;
        let dir = std::env::temp_dir().join(format!("np-mcfg-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("does-not-exist.gguf");
        assert!(get(&pool, &path).await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn list_returns_persisted_configs_in_order() {
        let pool = fresh_pool().await;
        let dir = std::env::temp_dir().join(format!("np-mcfg-list-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.gguf");
        let b = dir.join("b.gguf");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        upsert(&pool, &b, &ModelConfig::default()).await.unwrap();
        upsert(&pool, &a, &ModelConfig::default()).await.unwrap();
        let listed = list(&pool).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed[0].filename.ends_with("a.gguf"));
        assert!(listed[1].filename.ends_with("b.gguf"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
