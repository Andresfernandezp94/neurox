// Settings — small key/value store for engine-wide defaults that
// don't fit cleanly into the provider CRUD table. The first user of
// this is the explicit `default_provider` (the daemon used to just
// pick the first provider alphabetically; that broke when the user
// wanted a specific provider as default regardless of insertion
// order). The key/value shape lets future defaults land here without
// schema migrations.

use sqlx::SqlitePool;

pub async fn ensure_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS engine_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Read a single key from the settings store. Returns `Ok(None)`
/// when the key isn't set.
async fn load_key(pool: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM engine_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(v,)| if v.is_empty() { None } else { Some(v) }))
}

/// Persist a single key in the settings store. Pass `None` to clear.
async fn save_key(
    pool: &SqlitePool,
    key: &str,
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    match value {
        Some(v) => {
            sqlx::query(
                "INSERT INTO engine_settings (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(key)
            .bind(v)
            .execute(pool)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM engine_settings WHERE key = ?")
                .bind(key)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Read the explicitly-configured default provider id, if any. Returns
/// `Ok(None)` when no default has been configured yet (engine falls
/// back to alphabetical-first provider).
pub async fn load_default(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    load_key(pool, "default_provider").await
}

/// Persist the explicit default provider id. Pass `None` to clear.
pub async fn save_default(
    pool: &SqlitePool,
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    save_key(pool, "default_provider", value).await
}

/// Read the explicitly-configured default model id (e.g.
/// `mistral-medium-latest`). When `None`, falls back to the chosen
/// provider's `effective_model()`.
pub async fn load_default_model(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    load_key(pool, "default_model").await
}

/// Persist the explicit default model id. Pass `None` to clear.
pub async fn save_default_model(
    pool: &SqlitePool,
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    save_key(pool, "default_model", value).await
}
