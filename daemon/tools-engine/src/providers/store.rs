//! SQLite persistence for LLM providers.
//!
//! This module is the single point of contact with the `llm_providers` table
//! in the daemon's SQLite database. All reads and writes go through here.

use crate::config::{LlmProviderConfig, LlmProviderKind};
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Ensure the `llm_providers` table exists (idempotent migration).
/// Creates missing columns on existing tables (EP-0018-01).
pub async fn ensure_table(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS llm_providers (
            id                TEXT PRIMARY KEY,
            kind              TEXT NOT NULL,
            base_url          TEXT NOT NULL DEFAULT '',
            model             TEXT NOT NULL DEFAULT '',
            api_key_env       TEXT,
            extra_json        TEXT,
            local_command     TEXT,
            local_args_json   TEXT,
            local_model_path  TEXT,
            local_port        INTEGER,
            created_at        TEXT NOT NULL,
            updated_at        TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("failed to create llm_providers table: {e}"))?;

    // EP-0018-01: migrate existing tables that lack the local-service columns.
    // SQLite has no `ADD COLUMN IF NOT EXISTS` — guard by PRAGMA table_info.
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('llm_providers')")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("failed to inspect llm_providers columns: {e}"))?;

    let missing: &[(&str, &str)] = &[
        ("local_command", "TEXT"),
        ("local_args_json", "TEXT"),
        ("local_model_path", "TEXT"),
        ("local_port", "INTEGER"),
    ];
    for (name, ty) in missing {
        if !columns.iter().any(|c| c == name) {
            // `name` comes from the fixed internal list above — never from
            // user input. AssertSqlSafe is required by sqlx 0.9 for dynamic
            // SQL strings.
            let sql = format!("ALTER TABLE llm_providers ADD COLUMN {name} {ty}");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(pool)
                .await
                .map_err(|e| format!("failed to add column '{name}' to llm_providers: {e}"))?;
        }
    }
    Ok(())
}

/// List all providers ordered by `id` (lexicographic, deterministic).
pub async fn list(pool: &SqlitePool) -> Result<Vec<LlmProviderConfig>, String> {
    let rows: Vec<ProviderRow> = sqlx::query_as(
        "SELECT id, kind, base_url, model, api_key_env, extra_json,
                local_command, local_args_json, local_model_path, local_port,
                created_at, updated_at
         FROM llm_providers ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("failed to list providers: {e}"))?;

    rows.into_iter().map(row_to_config).collect()
}

/// Get a single provider by id.
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<LlmProviderConfig>, String> {
    let row: Option<ProviderRow> = sqlx::query_as(
        "SELECT id, kind, base_url, model, api_key_env, extra_json,
                local_command, local_args_json, local_model_path, local_port,
                created_at, updated_at
         FROM llm_providers WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("failed to get provider '{id}': {e}"))?;

    match row {
        Some(r) => Ok(Some(row_to_config(r)?)),
        None => Ok(None),
    }
}

/// Insert a new provider. Returns error if `id` already exists.
pub async fn insert(pool: &SqlitePool, p: &LlmProviderConfig) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let kind_str = kind_to_str(p.kind);
    let extra_json = if p.extra.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&p.extra).map_err(|e| format!("serialize extra: {e}"))?)
    };
    let args_json = local_args_json(p)?;

    sqlx::query(
        "INSERT INTO llm_providers (id, kind, base_url, model, api_key_env, extra_json,
                                    local_command, local_args_json, local_model_path, local_port,
                                    created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&p.id)
    .bind(kind_str)
    .bind(&p.base_url)
    .bind(&p.model)
    .bind(&p.api_key_env)
    .bind(&extra_json)
    .bind(&p.local_command)
    .bind(&args_json)
    .bind(&p.local_model_path)
    .bind(p.local_port.map(|p| p as i64))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint") {
            format!("provider with id '{}' already exists", p.id)
        } else {
            format!("failed to insert provider '{}': {e}", p.id)
        }
    })?;

    Ok(())
}

/// Update an existing provider. Returns error if `id` does not exist.
/// Only updates `base_url`, `model`, `api_key_env`, `extra` and the
/// EP-0018 local fields — `kind` is immutable.
pub async fn update(pool: &SqlitePool, id: &str, p: &LlmProviderConfig) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let extra_json = if p.extra.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&p.extra).map_err(|e| format!("serialize extra: {e}"))?)
    };
    let args_json = local_args_json(p)?;

    let result = sqlx::query(
        "UPDATE llm_providers SET base_url = ?, model = ?, api_key_env = ?, extra_json = ?,
                                  local_command = ?, local_args_json = ?, local_model_path = ?, local_port = ?,
                                  updated_at = ?
         WHERE id = ?",
    )
    .bind(&p.base_url)
    .bind(&p.model)
    .bind(&p.api_key_env)
    .bind(&extra_json)
    .bind(&p.local_command)
    .bind(&args_json)
    .bind(&p.local_model_path)
    .bind(p.local_port.map(|p| p as i64))
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| format!("failed to update provider '{id}': {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!("provider '{id}' not found"));
    }
    Ok(())
}

/// Delete a provider by id. Returns error if `id` does not exist.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM llm_providers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to delete provider '{id}': {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!("provider '{id}' not found"));
    }
    Ok(())
}

/// Count the number of providers in the store.
pub async fn count(pool: &SqlitePool) -> Result<i64, String> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM llm_providers")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("failed to count providers: {e}"))?;
    Ok(n)
}

// ─── Internal helpers ───────────────────────────────────────────────────────

type ProviderRow = (
    String,         // id
    String,         // kind
    String,         // base_url
    String,         // model
    Option<String>, // api_key_env
    Option<String>, // extra_json
    Option<String>, // local_command
    Option<String>, // local_args_json
    Option<String>, // local_model_path
    Option<i64>,    // local_port
    String,         // created_at
    String,         // updated_at
);

fn row_to_config(row: ProviderRow) -> Result<LlmProviderConfig, String> {
    let (
        id,
        kind_str,
        base_url,
        model,
        api_key_env,
        extra_json,
        local_command,
        local_args_json,
        local_model_path,
        local_port,
        _created,
        _updated,
    ) = row;
    let kind = str_to_kind(&kind_str)
        .ok_or_else(|| format!("unknown provider kind '{kind_str}' for id '{id}'"))?;
    let extra: HashMap<String, String> = match extra_json {
        Some(ref j) if !j.is_empty() => {
            serde_json::from_str(j).map_err(|e| format!("parse extra_json for '{id}': {e}"))?
        }
        _ => HashMap::new(),
    };
    let local_args: Vec<String> = match local_args_json {
        Some(ref j) if !j.is_empty() => {
            serde_json::from_str(j).map_err(|e| format!("parse local_args_json for '{id}': {e}"))?
        }
        _ => Vec::new(),
    };
    Ok(LlmProviderConfig {
        id,
        kind,
        base_url,
        model,
        api_key_env,
        extra,
        local_command,
        local_args,
        local_model_path,
        local_port: local_port.map(|p| p as u16),
    })
}

/// Serialize `local_args` as JSON (None when empty).
fn local_args_json(p: &LlmProviderConfig) -> Result<Option<String>, String> {
    if p.local_args.is_empty() {
        Ok(None)
    } else {
        serde_json::to_string(&p.local_args)
            .map(Some)
            .map_err(|e| format!("serialize local_args: {e}"))
    }
}

fn kind_to_str(kind: LlmProviderKind) -> &'static str {
    match kind {
        LlmProviderKind::Minimax => "minimax",
        LlmProviderKind::OpenaiCompat => "openai_compat",
        LlmProviderKind::Anthropic => "anthropic",
    }
}

fn str_to_kind(s: &str) -> Option<LlmProviderKind> {
    match s {
        "minimax" => Some(LlmProviderKind::Minimax),
        "openai_compat" => Some(LlmProviderKind::OpenaiCompat),
        "anthropic" => Some(LlmProviderKind::Anthropic),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn memory_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    fn local_provider() -> LlmProviderConfig {
        LlmProviderConfig {
            id: "local-llama".into(),
            kind: LlmProviderKind::OpenaiCompat,
            base_url: String::new(),
            model: "qwen2.5-1.5b-instruct".into(),
            api_key_env: None,
            extra: HashMap::new(),
            local_command: Some("llama-server".into()),
            local_args: vec![
                "--model".into(),
                "{{model_path}}".into(),
                "--port".into(),
                "{{port}}".into(),
            ],
            local_model_path: Some("~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf".into()),
            local_port: Some(11435),
        }
    }

    #[test]
    fn local_args_json_none_when_empty() {
        let p = LlmProviderConfig {
            local_args: Vec::new(),
            ..local_provider()
        };
        assert_eq!(local_args_json(&p).unwrap(), None);
    }

    #[test]
    fn local_args_json_serializes() {
        let p = local_provider();
        let json = local_args_json(&p).unwrap().unwrap();
        let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, p.local_args);
    }

    #[tokio::test]
    async fn store_roundtrip_local_fields() {
        // EP-0018-01 req 4.3: insert/get roundtrips the 4 local fields.
        let pool = memory_pool().await;
        ensure_table(&pool).await.unwrap();
        let p = local_provider();
        insert(&pool, &p).await.unwrap();

        let got = get(&pool, "local-llama").await.unwrap().unwrap();
        assert_eq!(got.id, p.id);
        assert_eq!(got.local_command, p.local_command);
        assert_eq!(got.local_args, p.local_args);
        assert_eq!(got.local_model_path, p.local_model_path);
        assert_eq!(got.local_port, p.local_port);
        assert_eq!(got.effective_base_url(), "http://127.0.0.1:11435/v1");

        let all = list(&pool).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].local_port, Some(11435));
    }

    #[tokio::test]
    async fn store_roundtrip_remote_backward_compat() {
        // EP-0018-01 req 4.2: a provider without local fields persists and
        // loads without the local columns being set.
        let pool = memory_pool().await;
        ensure_table(&pool).await.unwrap();
        let p = LlmProviderConfig {
            id: "openai".into(),
            kind: LlmProviderKind::OpenaiCompat,
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o".into(),
            api_key_env: Some("OPENAI_API_KEY".into()),
            extra: HashMap::new(),
            local_command: None,
            local_args: Vec::new(),
            local_model_path: None,
            local_port: None,
        };
        insert(&pool, &p).await.unwrap();
        let got = get(&pool, "openai").await.unwrap().unwrap();
        assert_eq!(got.local_command, None);
        assert!(got.local_args.is_empty());
        assert_eq!(got.local_port, None);
        assert_eq!(got.effective_base_url(), "https://api.openai.com/v1");
    }

    #[tokio::test]
    async fn ensure_table_migrates_old_table() {
        // EP-0018-01 req 4.4: an existing table without the local-service
        // columns is migrated (ALTER TABLE ADD COLUMN) and stays usable.
        let pool = memory_pool().await;
        sqlx::query(
            "CREATE TABLE llm_providers (
                id          TEXT PRIMARY KEY,
                kind        TEXT NOT NULL,
                base_url    TEXT NOT NULL DEFAULT '',
                model       TEXT NOT NULL DEFAULT '',
                api_key_env TEXT,
                extra_json  TEXT,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        ensure_table(&pool).await.unwrap();

        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('llm_providers')")
                .fetch_all(&pool)
                .await
                .unwrap();
        for expected in [
            "local_command",
            "local_args_json",
            "local_model_path",
            "local_port",
        ] {
            assert!(
                columns.contains(&expected.to_string()),
                "missing {expected}"
            );
        }

        // Existing table is now usable with local fields.
        let p = local_provider();
        insert(&pool, &p).await.unwrap();
        let got = get(&pool, "local-llama").await.unwrap().unwrap();
        assert_eq!(got.local_port, Some(11435));
    }

    #[tokio::test]
    async fn ensure_table_is_idempotent() {
        let pool = memory_pool().await;
        ensure_table(&pool).await.unwrap();
        ensure_table(&pool).await.unwrap();
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('llm_providers')")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(columns.contains(&"local_port".to_string()));
    }
}
