//! Public surface of the LLM-provider store.
//!
//! Mirrors the `ModelConfigStore` pattern: a thin wrapper around
//! `store::{list, get, insert, update, delete, count}` that holds the
//! `SqlitePool` and is injected into the engine as an `Arc`.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::LlmProviderConfig;
use crate::providers::store;

/// Run the schema migration on the given pool. Idempotent.
pub async fn ensure_table(pool: &SqlitePool) -> Result<(), String> {
    store::ensure_table(pool).await
}

/// Async-friendly handle around the `llm_providers` SQLite table.
#[derive(Clone)]
pub struct ProviderStore {
    pool: Arc<SqlitePool>,
}

impl ProviderStore {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    /// Borrow the underlying pool handle.
    pub fn pool(&self) -> Arc<SqlitePool> {
        Arc::clone(&self.pool)
    }

    pub async fn list(&self) -> Result<Vec<LlmProviderConfig>, String> {
        store::list(&self.pool).await
    }

    pub async fn get(&self, id: &str) -> Result<Option<LlmProviderConfig>, String> {
        store::get(&self.pool, id).await
    }

    pub async fn insert(&self, p: &LlmProviderConfig) -> Result<(), String> {
        store::insert(&self.pool, p).await
    }

    pub async fn update(&self, id: &str, p: &LlmProviderConfig) -> Result<(), String> {
        store::update(&self.pool, id, p).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        store::delete(&self.pool, id).await
    }

    pub async fn count(&self) -> Result<i64, String> {
        store::count(&self.pool).await
    }

    /// EP-0004 wave 1: seed SQLite from a YAML list if the store is
    /// empty. Returns the number of providers inserted (0 if the store
    /// already had rows or if `yaml_providers` was empty).
    pub async fn bootstrap_from_yaml(
        &self,
        yaml_providers: &[LlmProviderConfig],
    ) -> Result<usize, String> {
        let count = self.count().await?;
        if count > 0 {
            return Ok(0);
        }
        if yaml_providers.is_empty() {
            return Ok(0);
        }
        let mut inserted = 0;
        for p in yaml_providers {
            self.insert(p).await?;
            inserted += 1;
        }
        Ok(inserted)
    }
}
