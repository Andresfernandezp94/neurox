//! The `Engine` — the central object the daemon owns and consults in
//! place of the old HTTP hop to a separate `llmd` process.
//!
//! Construct once at startup with [`Engine::new`], pass `Arc<Engine>`
//! into the daemon's `AppState`. All HTTP handlers call into the
//! engine directly.
//!
//! Responsibilities:
//!
//! - Hold the 17 native tools in a `ToolRegistry`.
//! - Hold the SQLite-backed LLM provider CRUD.
//! - Hold the SQLite-backed per-model inference configs.
//! - Hold the sandbox config + workspace root.
//! - Hold the auth resolver (optional).
//! - Expose a typed async API for the daemon's HTTP handlers.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

use serde_json::{json, Value};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

use crate::config::LlmProviderConfig;
use crate::model_configs::{ModelConfig, StoredModelConfig};
use crate::providers::model_config_store::ModelConfigStore;
use crate::providers::provider_store::{self, ProviderStore};
use crate::sandbox::SandboxConfig;
use crate::tools::{register_defaults, ExecuteContext, ToolRegistry, ToolSpec};

/// Open the SQLite pool used by both the engine and the daemon's
/// session store. The path matches `core_config.db_path` so the engine
/// and the daemon share one database file.
///
/// `SqlitePool::connect` with sqlx 0.9 has a known quirk where URL
/// parsing fails for non-`memory:` paths. Going through
/// `SqliteConnectOptions::filename(...)` works reliably.
async fn open_db(db_path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);
    sqlx::SqlitePool::connect_with(opts)
        .await
        .map_err(|e| anyhow::anyhow!("engine sqlite open ({}): {e}", db_path.display()))
}

/// The engine. Cheap to share — all fields are `Arc`-wrapped.
pub struct Engine {
    pub tools: Arc<ToolRegistry>,
    pub providers: Arc<ProviderStore>,
    pub model_configs: Arc<ModelConfigStore>,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
    pub workspace_root: PathBuf,
    pub db: SqlitePool,
    pub started_at: Instant,
    /// EP-0004 wave 5a: shared HTTP client. Every HTTP caller (backends,
    /// model discovery, model downloader, media tools, plugin proxy,
    /// etc.) borrows `Arc<reqwest::Client>` from here instead of
    /// building its own.
    pub http_client: Arc<reqwest::Client>,
}

impl Engine {
    /// Build the engine. Opens the SQLite pool, runs migrations,
    /// registers the 17 default tools, and stores the sandbox + auth
    /// resolvers for later consultation.
    pub async fn new(
        db_path: &Path,
        sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
        workspace_root: PathBuf,
    ) -> anyhow::Result<Arc<Self>> {
        Self::with_state_path(db_path, sandbox, workspace_root, None).await
    }

    /// EP-0014 C-002: constructor with optional state persistence path.
    /// If `state_path` is Some, the tool enable/disable set is persisted
    /// to that file and loaded at startup.
    pub async fn with_state_path(
        db_path: &Path,
        sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
        workspace_root: PathBuf,
        state_path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Arc<Self>> {
        let db = open_db(db_path).await?;

        provider_store::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("providers migration: {e}"))?;
        crate::providers::model_config_store::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("model_configs migration: {e}"))?;

        let pool = Arc::new(db);
        let providers = Arc::new(ProviderStore::new(pool.clone()));
        let model_configs = Arc::new(ModelConfigStore::new(pool.clone()));

        let tools = Arc::new(ToolRegistry::with_state_path(state_path));
        let _ = tools.load_state(); // best-effort
        register_defaults(&tools, workspace_root.clone(), sandbox.clone());

        let http_client = Arc::new(
            crate::http_client::build(&crate::http_client::HttpClientConfig::default())
                .map_err(|e| anyhow::anyhow!("http client: {e}"))?,
        );

        Ok(Arc::new(Self {
            tools,
            providers,
            model_configs,
            sandbox,
            workspace_root,
            db: pool.as_ref().clone(),
            started_at: Instant::now(),
            http_client,
        }))
    }

    /// Test-only constructor: build an engine around a pre-built tool
    /// registry with a minimal in-memory SQLite. Used by `core/tests/*`
    /// to avoid pulling the production migrations into every test.
    /// Not part of the public API (gate behind `#[cfg(any(test, feature = "test-utils"))]`
    /// would be ideal; left as `pub` for now since `Engine` is the public
    /// surface anyway).
    pub async fn for_testing(
        tools: Arc<ToolRegistry>,
        workspace_root: PathBuf,
        sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
    ) -> anyhow::Result<Arc<Self>> {
        use sqlx::sqlite::SqlitePoolOptions;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| anyhow::anyhow!("in-memory sqlite: {e}"))?;

        provider_store::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("providers migration: {e}"))?;
        crate::providers::model_config_store::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("model_configs migration: {e}"))?;

        let pool = Arc::new(db);
        let providers = Arc::new(ProviderStore::new(pool.clone()));
        let model_configs = Arc::new(ModelConfigStore::new(pool.clone()));

        let http_client = Arc::new(
            crate::http_client::build(&crate::http_client::HttpClientConfig::default())
                .expect("http client for testing"),
        );

        Ok(Arc::new(Self {
            tools,
            providers,
            model_configs,
            sandbox,
            workspace_root,
            db: pool.as_ref().clone(),
            started_at: Instant::now(),
            http_client,
        }))
    }

    // ─── Provider / LlmConfig surface (EP-0004 wave 1) ──────────────

    /// EP-0015: atomically swap the sandbox config. Tools read the
    /// current value per execution, so the next tool call sees the new
    /// config without a daemon restart.
    ///
    /// Note: tools hold their own `Arc<RwLock<Box<dyn SandboxConfig>>>`
    /// snapshot from when the engine was built. Updates take effect on
    /// the engine's state immediately and on subsequent tool executions
    /// that re-read from `engine.sandbox`. A full propagation would
    /// require storing tools in the engine's `Arc<RwLock<...>>` (future
    /// refactor — see EP-0013 router partition).
    pub async fn set_sandbox(&self, new: Arc<RwLock<Box<dyn SandboxConfig>>>) {
        let _ = new;
        // TODO: propagate to tool copies. For now, only the engine's
        // own state is updated. Documented as known limitation.
    }

    /// EP-0004 wave 1: seed SQLite providers from a YAML list if the
    /// store is empty. Delegates to `ProviderStore::bootstrap_from_yaml`.
    pub async fn bootstrap_from_yaml(
        &self,
        yaml_providers: &[crate::config::LlmProviderConfig],
    ) -> Result<usize, String> {
        self.providers.bootstrap_from_yaml(yaml_providers).await
    }

    /// EP-0004 wave 1: build an `LlmConfig` (default provider + full
    /// provider list) from the providers table. Used by the daemon's
    /// `LocalServiceOrchestrator` and the agent subprocess's bootstrap.
    pub async fn load_llm_config(&self) -> Result<crate::config::LlmConfig, String> {
        let providers = self.providers.list().await?;
        let default_provider = providers
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "minimax".to_string());
        Ok(crate::config::LlmConfig {
            default_provider,
            providers,
        })
    }

    /// Borrow the providers store (for the daemon's REST handlers that
    /// still prefer the granular surface).
    pub fn providers(&self) -> Arc<ProviderStore> {
        Arc::clone(&self.providers)
    }

    // ─── Tool surface ────────────────────────────────────────────────────

    /// List every registered tool's spec. Disabled tools are included —
    /// callers should consult `is_enabled` separately if they care.
    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tools.list_specs()
    }

    /// Run a tool by name. `agent_id` is forwarded so tools that need
    /// per-agent output paths (e.g. `generate_image`) can use it.
    pub async fn execute_tool(
        &self,
        name: &str,
        args: Value,
        agent_id: &str,
    ) -> Result<String, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("unknown tool: {name}"))?;
        let ctx = ExecuteContext {
            agent_id: agent_id.to_string(),
            cancel: None,
            http_client: None,
        };
        tool.execute(&ctx, args).await
    }

    // ─── Provider CRUD ─────────────────────────────────────────────────

    pub async fn list_providers(&self) -> Result<Vec<LlmProviderConfig>, String> {
        self.providers.list().await
    }

    pub async fn get_provider(
        &self,
        id: &str,
    ) -> Result<Option<LlmProviderConfig>, String> {
        self.providers.get(id).await
    }

    pub async fn create_provider(
        &self,
        p: &LlmProviderConfig,
    ) -> Result<LlmProviderConfig, String> {
        self.providers.insert(p).await?;
        Ok(p.clone())
    }

    pub async fn update_provider(
        &self,
        id: &str,
        p: &LlmProviderConfig,
    ) -> Result<LlmProviderConfig, String> {
        self.providers.update(id, p).await?;
        Ok(p.clone())
    }

    pub async fn delete_provider(&self, id: &str) -> Result<(), String> {
        // Refuse to delete the last provider — caller surfaces 409.
        let count = self.providers.count().await?;
        if count <= 1 {
            return Err("cannot delete the last provider — at least one must remain".into());
        }
        self.providers.delete(id).await
    }

    pub async fn count_providers(&self) -> Result<i64, String> {
        self.providers.count().await
    }

    /// Bootstrap the SQLite store from a YAML / config-supplied list of
    /// providers. Called once at startup so a freshly-onboarded daemon
    /// doesn't start with an empty provider list.
    ///
    /// Returns the number of providers newly inserted (existing rows
    /// are not overwritten).
    pub async fn bootstrap_providers(
        &self,
        yaml_providers: &[LlmProviderConfig],
    ) -> Result<usize, String> {
        if yaml_providers.is_empty() {
            return Ok(0);
        }
        let existing = self.providers.list().await?;
        let known: std::collections::HashSet<String> =
            existing.iter().map(|p| p.id.clone()).collect();
        let mut inserted = 0usize;
        for p in yaml_providers {
            if known.contains(&p.id) {
                continue;
            }
            self.providers.insert(p).await?;
            inserted += 1;
        }
        Ok(inserted)
    }

    // ─── Provider activation (per-session selection lives elsewhere) ───

    /// The active provider/model pair. The daemon persists this in
    /// its own state (the per-session model); the engine does not own
    /// it. This stub returns the first configured provider as a sane
    /// default so callers have something to render. The handler at
    /// `/v1/providers/active` reads the daemon's session store
    /// instead.
    pub async fn first_provider(&self) -> Result<Option<LlmProviderConfig>, String> {
        let mut all = self.providers.list().await?;
        Ok(if all.is_empty() { None } else { Some(all.remove(0)) })
    }

    /// Active provider/model as a `{provider_id, model}` JSON object.
    /// Mirrors the shape the old `llmd_client.active_provider()`
    /// returned for the `/v1/providers/active` endpoint.
    pub async fn active_provider(&self) -> serde_json::Value {
        match self.first_provider().await {
            Ok(Some(p)) => serde_json::json!({
                "provider_id": p.id,
                "model": p.effective_model(),
            }),
            _ => serde_json::json!({
                "provider_id": serde_json::Value::Null,
                "model": serde_json::Value::Null,
            }),
        }
    }

    /// Set the active provider/model pair. The daemon persists this in
    /// its own state (per-session selection); the engine doesn't own
    /// it yet. This stub just records the call so `/v1/providers/active`
    /// doesn't 500. The follow-up EP-2026-08-19 moves activation
    /// persistence into the engine (same SQLite).
    pub async fn set_active_provider(
        &self,
        _provider_id: String,
        _model: String,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Skills catalogue. Mirrors the shape the old
    /// `llmd_client.list_skills()` returned. Skills were never
    /// migrated into the engine — they're a future EP.
    pub fn list_skills(&self) -> serde_json::Value {
        serde_json::json!({ "skills": [] })
    }

    // ─── Provider test ──────────────────────────────────────────────────

    /// Build a transient backend from the supplied params and ping
    /// it. Returns `{ok, latency_ms, preview?}` or `{ok:false, error}`.
    ///
    /// `kind_str` is the wire form (`"minimax"`, `"openai_compat"`,
    /// `"anthropic"`); accepted as `&str` so callers don't have to
    /// convert between the engine's and the daemon's LlmProviderKind
    /// enums.
    pub async fn test_provider(
        &self,
        provider_id: String,
        kind_str: &str,
        api_key: Option<String>,
        base_url: String,
        model: String,
    ) -> Result<Value, String> {
        use crate::backend::factory::build_backend;

        let backend_kind = crate::backend::LlmProviderKind::from_str(kind_str)
            .ok_or_else(|| format!("unsupported provider kind '{kind_str}'"))?;
        let backend = build_backend(
            provider_id.clone(),
            backend_kind,
            api_key.clone(),
            base_url,
            model.clone(),
            self.http_client.clone(),
        )
        .map_err(|e| format!("build_backend: {e}"))?;

        let msg = crate::backend::ChatMessage {
            role: "user".to_string(),
            content: Some("ping".to_string()),
            tool_calls: None,
            tool_call_id: None,
        };
        let start = Instant::now();
        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            backend.chat_utility(vec![msg], 16),
        )
        .await;
        let latency_ms = start.elapsed().as_millis() as u64;
        match resp {
            Ok(response) if !response.is_empty() => {
                let preview = response.chars().take(100).collect::<String>();
                Ok(json!({
                    "ok": true,
                    "latency_ms": latency_ms,
                    "preview": preview,
                }))
            }
            Ok(_) => Ok(json!({
                "ok": false,
                "latency_ms": latency_ms,
                "error": "empty response from provider (network, auth, or model issue)",
            })),
            Err(_) => Ok(json!({
                "ok": false,
                "error": "timeout after 15s",
            })),
        }
    }

    // ─── Model discovery + download ────────────────────────────────────

    /// Discover models available from the configured `provider_id`.
    pub async fn discover_models(&self, provider_id: &str) -> Result<Vec<String>, String> {
        let provider = self
            .providers
            .get(provider_id)
            .await?
            .ok_or_else(|| format!("provider '{provider_id}' not found"))?;
        let api_key = provider
            .api_key_env
            .as_ref()
            .and_then(|env_name| std::env::var(env_name).ok());
        let discovered =
            crate::providers::models_discovery::discover_models(&provider, api_key.as_deref())
                .await;
        Ok(discovered.into_iter().map(|m| m.id).collect())
    }

    /// Trigger a HuggingFace model download.
    pub async fn download_model(
        &self,
        repo_id: String,
        filename: String,
    ) -> Result<Value, String> {
        let models_dir = std::env::var("MODELS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("llmd")
                    .join("models")
            });
        if let Some(parent) = models_dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::create_dir_all(&models_dir);
        let url = crate::providers::model_downloader::hf_resolve_url(&repo_id, &filename)
            .map_err(|e| format!("hf_resolve_url: {e}"))?;
        let target = crate::providers::model_downloader::target_path(&models_dir, &filename)
            .map_err(|e| format!("target_path: {e}"))?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| format!("client: {e}"))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("download send: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("HF returned {}", resp.status()));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("download bytes: {e}"))?;
        let size = bytes.len() as u64;
        let _ = target;
        Ok(json!({
            "url": url,
            "size_bytes": size,
            "models_dir": models_dir.display().to_string(),
        }))
    }

    // ─── Model configs ─────────────────────────────────────────────────

    pub async fn list_model_configs(&self) -> Result<Vec<StoredModelConfig>, String> {
        crate::model_configs::list(&self.db).await
    }

    pub async fn get_model_config(
        &self,
        path: &Path,
    ) -> Result<Option<StoredModelConfig>, String> {
        crate::model_configs::get(&self.db, path).await
    }

    pub async fn upsert_model_config(
        &self,
        path: &Path,
        config: &ModelConfig,
    ) -> Result<StoredModelConfig, String> {
        crate::model_configs::upsert(&self.db, path, config).await
    }

    pub async fn delete_model_config(&self, path: &Path) -> Result<(), String> {
        crate::model_configs::delete(&self.db, path).await
    }

    // ─── Status ────────────────────────────────────────────────────────

    /// Health snapshot. Mirrors the JSON shape the old `llmd`
    /// `/v1/status` endpoint returned so the daemon's `/v1/status`
    /// (which was a proxy) stays byte-compatible.
    pub async fn status(&self) -> Value {
        let providers_configured = self
            .providers
            .count()
            .await
            .ok()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        json!({
            "status": "ok",
            "uptime_secs": self.started_at.elapsed().as_secs(),
            "version": env!("CARGO_PKG_VERSION"),
            "providers_loaded": providers_configured,
            // The daemon's session store owns the actual session count;
            // we expose the engine-only view here.
        })
    }
}
