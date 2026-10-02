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
    /// Explicit default provider id (when set, overrides the
    /// alphabetical-first provider in `first_provider`). Writable via
    /// `set_default_provider` (called by the daemon from
    /// `core_config.llm.default_provider` at startup, or by the
    /// `PUT /v1/llm/providers/active` endpoint at runtime).
    default_provider: Arc<RwLock<Option<String>>>,
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
        crate::providers::settings::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("settings migration: {e}"))?;
        // Preferencias de LLM por usuario. Va aparte de `engine_settings`
        // porque esa tabla es key/value global: el default de provider es
        // config DEL USUARIO y dos usuarios no pueden pisarse.
        crate::providers::user_llm_pref::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("user_llm_prefs migration: {e}"))?;

        let pool = Arc::new(db);
        let providers = Arc::new(ProviderStore::new(pool.clone()));
        let model_configs = Arc::new(ModelConfigStore::new(pool.clone()));

        let tools = Arc::new(ToolRegistry::with_state_path(state_path));
        let _ = tools.load_state(); // best-effort
        register_defaults(&tools, workspace_root.clone(), sandbox.clone());

        // Pre-load the explicit default provider from the persisted
        // settings table (if any). Falls back to None → engine picks
        // alphabetical-first provider.
        let default_provider = match crate::providers::settings::load_default(&pool).await {
            Ok(id) => id,
            Err(_) => None,
        };
        let default_provider = Arc::new(RwLock::new(default_provider));

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
            default_provider,
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
        crate::providers::user_llm_pref::ensure_table(&db)
            .await
            .map_err(|e| anyhow::anyhow!("user_llm_prefs migration: {e}"))?;

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
            default_provider: Arc::new(RwLock::new(None)),
        }))
    }

    // ─── Provider / LlmConfig surface (EP-0004 wave 1) ──────────────

    /// EP-0015: reemplaza el sandbox activo. Las tools leen el valor
    /// actual en cada ejecucion, asi que el siguiente tool call ya ve la
    /// config nueva, sin reiniciar el daemon.
    ///
    /// Se ESCRIBE DENTRO del `RwLock` compartido en vez de cambiar el
    /// `Arc`. Es lo que hace que la propagacion sea gratis: `register_defaults`
    /// le pasa a cada tool un `sandbox.clone()` de ESTE mismo Arc, asi que
    /// todas comparten la celda de memoria. Reemplazar el Arc dejaria a las
    /// tools apuntando al viejo, que es justo el bug que habia antes.
    ///
    /// Antes el cuerpo era `let _ = new;`: descartaba el valor y el
    /// comentario de al lado decia que solo se actualizaba el estado del
    /// engine, lo cual era falso. `PUT /v1/sandbox` validaba los paths,
    /// construia la config, y no pasaba nada.
    pub async fn set_sandbox(&self, new: Box<dyn SandboxConfig>) {
        let mut guard = self.sandbox.write().await;
        *guard = new;
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
        let explicit_default = self.default_provider.read().await.clone();
        let default_provider = explicit_default
            .or_else(|| providers.first().map(|p| p.id.clone()))
            .unwrap_or_else(|| "minimax".to_string());
        // Persisted default model override (optional). When set,
        // overrides `effective_model()` for the active provider.
        let default_model =
            crate::providers::settings::load_default_model(&self.db).await.ok().flatten();
        Ok(crate::config::LlmConfig {
            default_provider,
            default_model,
            providers,
        })
    }

    /// Set the explicit default provider id (called by the daemon at
    /// startup from `core_config.llm.default_provider`, or by the
    /// `PUT /v1/llm/providers/active` endpoint at runtime). When `Some`,
    /// `first_provider()` honors this over the alphabetical first
    /// provider. When `None` (default), falls back to the first
    /// provider in the store (legacy behavior).
    pub async fn set_default_provider(&self, id: Option<String>) -> Result<(), String> {
        // Validate the id exists if provided so a typo doesn't leave
        // the engine in a state where everything errors out.
        if let Some(ref candidate) = id {
            let providers = self.providers.list().await?;
            if !providers.iter().any(|p| &p.id == candidate) {
                return Err(format!("unknown provider: {candidate}"));
            }
        }
        crate::providers::settings::save_default(&self.db, id.as_deref())
            .await
            .map_err(|e| format!("settings save: {e}"))?;
        let mut guard = self.default_provider.write().await;
        *guard = id;
        Ok(())
    }

    /// Set the explicit default model id (e.g. `mistral-medium-latest`).
    /// Persisted in `engine_settings.default_model`. When `None`, the
    /// chosen provider's `effective_model()` is used.
    pub async fn set_default_model(&self, model: Option<String>) -> Result<(), String> {
        crate::providers::settings::save_default_model(&self.db, model.as_deref())
            .await
            .map_err(|e| format!("settings save: {e}"))
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
            workspace: None,
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
        if all.is_empty() {
            return Ok(None);
        }
        // Honour the explicit default provider (set via
        // `set_default_provider` or loaded from `engine_settings`).
        let explicit = self.default_provider.read().await.clone();
        if let Some(id) = explicit {
            if let Some(idx) = all.iter().position(|p| p.id == id) {
                return Ok(Some(all.remove(idx)));
            }
            // Explicit id no longer exists in the store (provider was
            // deleted) — fall back to the first provider.
            tracing::warn!(
                default_provider = %id,
                "explicit default provider not in store; falling back to first"
            );
        }
        Ok(Some(all.remove(0)))
    }

    /// Active provider/model as a `{provider_id, model}` JSON object.
    /// Mirrors the shape the old `llmd_client.active_provider()`
    /// returned for the `/v1/providers/active` endpoint.
    ///
    /// `default_model` (if set via `set_default_model` or persisted
    /// in `engine_settings.default_model`) overrides the provider's
    /// `effective_model()` — that's how the daemon-wide "default
    /// model" flows to `/v1/llm/providers` and into newly-created
    /// sessions.
    pub async fn active_provider(&self) -> serde_json::Value {
        let explicit_model = match crate::providers::settings::load_default_model(&self.db).await {
            Ok(m) => m,
            Err(_) => None,
        };
        match self.first_provider().await {
            Ok(Some(p)) => {
                let model = explicit_model.unwrap_or_else(|| p.effective_model());
                serde_json::json!({
                    "provider_id": p.id,
                    "model": model,
                })
            }
            _ => serde_json::json!({
                "provider_id": serde_json::Value::Null,
                "model": serde_json::Value::Null,
            }),
        }
    }

    /// Set the active provider/model pair. The daemon persists this in
    /// its own state (per-session selection); the engine doesn't own
    /// it yet. This stub just records the call so `/v1/providers/active`
    /// Persist the daemon-wide default provider/model. Both fields
    /// are honored by `first_provider` (explicit id wins over
    /// alphabetical first) and `active_provider` (explicit model
    /// wins over the provider's `effective_model()`).
    pub async fn set_active_provider(
        &self,
        provider_id: String,
        model: String,
    ) -> Result<(), String> {
        self.set_default_provider(Some(provider_id.clone())).await?;
        self.set_default_model(Some(model)).await?;
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

#[cfg(test)]
mod sandbox_propagation_tests {
    //! El sandbox tiene que propagarse a las tools.
    //!
    //! Estas tools se construyen con un `Arc` al sandbox y lo leen en cada
    //! ejecucion. Como todas comparten la misma celda, actualizar el sandbox
    //! se Propaga Es escribir adentro de esa celda.
    //!
    //! Antes `set_sandbox` hacia `let _ = new;` (descartaba el valor) con un
    //! comentario que decia que solo se actualizaba el estado del engine.
    //! Ademas el engine y el `WorkspaceLayer` tenian DOS Arcs distintos
    //! construidos desde los mismos valores iniciales, asi que configurar el
    //! sandbox desde la UI no llegaba a ninguna tool: estas seguian leyendo
    //! su propio Arc, que nadie mutaba. Editar los paths no hacia nada.

    use super::*;
    use crate::sandbox::SandboxConfig;
    use crate::tools::read::read_file::ReadFileTool;
    use crate::tools::Tool;
    use crate::tools::{register_defaults, ToolRegistry};
    use std::path::{Path, PathBuf};
    use tokio::sync::RwLock;

    /// Sandbox de test que se puede reconfigurar, para poder observar si el
    /// cambio llego a la tool o no.
    ///
    /// `readable_paths_resolved` es lo que consulta `ReadFileTool` para
    /// resolver un path. Si no se overridea, el default del trait devuelve
    /// vacio y la tool solo acepta el workspace root.
    struct SwitchableSandbox {
        readable: Vec<PathBuf>,
    }

    impl SandboxConfig for SwitchableSandbox {
        fn enabled(&self) -> bool {
            true
        }
        fn is_writable(&self, _p: &Path) -> bool {
            false
        }
        fn is_readable(&self, path: &Path) -> bool {
            self.readable.iter().any(|r| path.starts_with(r))
        }
        fn readable_paths_resolved(&self, _workspace_root: &Path) -> Vec<PathBuf> {
            self.readable.clone()
        }
    }

    fn ctx() -> crate::ExecuteContext {
        crate::ExecuteContext {
            agent_id: "test".into(),
            workspace: None,
            cancel: None,
            http_client: None,
        }
    }

    /// El caso que reportaba el usuario: la UI cambia los paths y la tool
    /// tiene que verlos en la siguiente ejecucion, sin reiniciar nada.
    #[tokio::test]
    async fn a_tool_sees_the_sandbox_change_made_after_it_was_built() {
        // El workspace y el archivo estan en directorios distintos, asi que
        // con la lista de legibles vacia la tool lo rechaza.
        let workspace = tempfile::TempDir::new().unwrap();
        let fuera = tempfile::TempDir::new().unwrap();
        let file = fuera.path().join("dato.txt");
        std::fs::write(&file, "hola").unwrap();

        let sandbox: Arc<RwLock<Box<dyn SandboxConfig>>> = Arc::new(RwLock::new(Box::new(
            SwitchableSandbox { readable: vec![] },
        )));

        // La tool se construye con el sandbox cerrado. Mismo camino que
        // produccion: `register_defaults` le pasa un clon del Arc.
        let read_file = ReadFileTool {
            workspace_root: workspace.path().to_path_buf(),
            sandbox: sandbox.clone(),
        };

        let antes = read_file
            .execute(&ctx(), serde_json::json!({ "path": file.to_string_lossy() }))
            .await;
        assert!(
            antes.is_err(),
            "precondicion: sin readable_paths la tool tiene que rechazar, dio: {:?}",
            antes
        );

        // Esto es lo que hace `PUT /v1/sandbox`: agregar el path.
        let tools = Arc::new(ToolRegistry::with_state_path(None));
        register_defaults(&tools, workspace.path().to_path_buf(), sandbox.clone());
        let engine =
            Engine::for_testing(tools, workspace.path().to_path_buf(), sandbox.clone())
                .await
                .unwrap();
        engine
            .set_sandbox(Box::new(SwitchableSandbox {
                readable: vec![fuera.path().to_path_buf()],
            }))
            .await;

        // La MISMA instancia de la tool, que antes rechazaba, tiene que poder
        // leer ahora. Si `set_sandbox` reemplazara el Arc en vez de escribir
        // en la celda, esta seguiria viendo el viejo y fallaria.
        let despues = read_file
            .execute(&ctx(), serde_json::json!({ "path": file.to_string_lossy() }))
            .await;
        // `read_file` devuelve las lineas numeradas (`1| ...`), asi que se
        // compara contra ese formato y no contra el contenido crudo.
        let salida = despues.expect("la tool tiene que poder leer ahora");
        assert!(
            salida.contains("hola"),
            "la tool tiene que ver el sandbox nuevo: el cambio se aplico por el \
             Arc que compartio, no por un Arc nuevo. Salida: {salida:?}"
        );
    }

    /// El valor queda en la celda compartida, que es la misma que leen las
    /// tools. Antes el valor se descartaba.
    #[tokio::test]
    async fn set_sandbox_writes_into_the_shared_cell() {
        let sandbox: Arc<RwLock<Box<dyn SandboxConfig>>> = Arc::new(RwLock::new(Box::new(
            SwitchableSandbox { readable: vec![] },
        )));
        let tools = Arc::new(ToolRegistry::with_state_path(None));
        register_defaults(&tools, PathBuf::from("/tmp"), sandbox.clone());
        let engine = Engine::for_testing(tools, PathBuf::from("/tmp"), sandbox.clone())
            .await
            .unwrap();

        let granted = PathBuf::from("/tmp");
        engine
            .set_sandbox(Box::new(SwitchableSandbox {
                readable: vec![granted.clone()],
            }))
            .await;

        // Se lee por el Arc de las tools, no por el del engine.
        assert!(
            sandbox.read().await.is_readable(&granted),
            "el valor tiene que estar en la celda compartida"
        );
    }
}

#[cfg(test)]
mod workspace_scope_tests {
    //! Aislamiento por workspace: el root y el sandbox de la LLAMADA
    //! mandan sobre los valores fijos con los que se construyo el tool.
    //!
    //! Los tools de filesystem guardan `workspace_root: PathBuf` y
    //! `sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>` como valores fijos
    //! desde su construccion (`register_defaults`), y hay UN solo registro
    //! de tools. Sin el contexto no habia manera de que dos workspaces
    //! distintos usaran el mismo tool con permisos distintos.
    //!
    //! `ExecuteContext.workspace` es lo que lo resuelve: lo arma el daemon,
    //! que es el que sabe que workspace tiene la sesion que llamo.

    use super::*;
    use crate::sandbox::SandboxConfig as SandboxTrait;
    use crate::tools::read::read_file::ReadFileTool;
    use crate::tools::Tool;
    use std::path::{Path, PathBuf};
    use crate::tools::{ExecuteContext, WorkspaceScope};

    /// Sandbox parametrizable por path legible.
    struct ReadableOnly(Vec<PathBuf>);

    impl SandboxTrait for ReadableOnly {
        fn enabled(&self) -> bool {
            true
        }
        fn is_writable(&self, _p: &Path) -> bool {
            false
        }
        fn is_readable(&self, p: &Path) -> bool {
            self.0.iter().any(|r| p.starts_with(r))
        }
        fn readable_paths_resolved(&self, _ws: &Path) -> Vec<PathBuf> {
            self.0.clone()
        }
    }

    fn ctx() -> ExecuteContext {
        ExecuteContext {
            agent_id: "test".into(),
            workspace: None,
            cancel: None,
            http_client: None,
        }
    }

    fn scope(root: &Path, sandbox: ReadableOnly) -> WorkspaceScope {
        WorkspaceScope {
            id: Some("w".into()),
            root: root.to_path_buf(),
            sandbox: Arc::new(tokio::sync::RwLock::new(Box::new(sandbox))),
        }
    }

    /// El root del contexto pisa el del tool. El tool se construye apuntando
    /// a `propio` con el sandbox cerrado, pero la llamada va con el root
    /// `ajeno`: leer de `ajeno` tiene que andar y leer de `propio` no.
    #[tokio::test]
    async fn el_root_del_contexto_pisa_el_del_tool() {
        let propio = tempfile::TempDir::new().unwrap();
        let ajeno = tempfile::TempDir::new().unwrap();
        let en_ajeno = ajeno.path().join("dato.txt");
        let en_propio = propio.path().join("dato.txt");
        std::fs::write(&en_ajeno, "ajeno").unwrap();
        std::fs::write(&en_propio, "propio").unwrap();

        let vacio: Arc<RwLock<Box<dyn SandboxTrait>>> =
            Arc::new(RwLock::new(Box::new(ReadableOnly(vec![]))));
        let tool = ReadFileTool {
            workspace_root: propio.path().to_path_buf(),
            sandbox: vacio,
        };

        let mut c = ctx();
        c.workspace = Some(scope(ajeno.path(), ReadableOnly(vec![ajeno.path().to_path_buf()])));

        let leido = tool
            .execute(&c, serde_json::json!({ "path": en_ajeno.to_string_lossy() }))
            .await
            .expect("tiene que leer del root del contexto");
        assert!(leido.contains("ajeno"), "salida: {leido:?}");

        let fuera = tool
            .execute(&c, serde_json::json!({ "path": en_propio.to_string_lossy() }))
            .await;
        assert!(
            fuera.is_err(),
            "el root del tool ya no manda: su directorio deberia estar fuera"
        );
    }

    /// Dos workspaces con permisos distintos no se pisan: el sandbox que
    /// aplica es el del contexto, no el que el tool tiene guardado.
    #[tokio::test]
    async fn cada_workspace_manda_con_su_sandbox() {
        let dir_a = tempfile::TempDir::new().unwrap();
        let dir_b = tempfile::TempDir::new().unwrap();
        let en_a = dir_a.path().join("a.txt");
        let en_b = dir_b.path().join("b.txt");
        std::fs::write(&en_a, "A").unwrap();
        std::fs::write(&en_b, "B").unwrap();

        // El tool arranca con un sandbox que no deja leer nada.
        let cerrado: Arc<RwLock<Box<dyn SandboxTrait>>> =
            Arc::new(RwLock::new(Box::new(ReadableOnly(vec![]))));
        let tool = ReadFileTool {
            workspace_root: dir_a.path().to_path_buf(),
            sandbox: cerrado,
        };

        // Llamada 1: workspace con root de A, legible solo A.
        let mut c_a = ctx();
        c_a.workspace = Some(scope(
            dir_a.path(),
            ReadableOnly(vec![dir_a.path().to_path_buf()]),
        ));
        assert!(tool
            .execute(&c_a, serde_json::json!({ "path": en_a.to_string_lossy() }))
            .await
            .is_ok());
        assert!(
            tool.execute(&c_a, serde_json::json!({ "path": en_b.to_string_lossy() }))
                .await
                .is_err(),
            "A no puede leer B"
        );

        // Llamada 2: MISMO tool, workspace con root de B, legible solo B.
        let mut c_b = ctx();
        c_b.workspace = Some(scope(
            dir_b.path(),
            ReadableOnly(vec![dir_b.path().to_path_buf()]),
        ));
        assert!(tool
            .execute(&c_b, serde_json::json!({ "path": en_b.to_string_lossy() }))
            .await
            .is_ok());
        assert!(
            tool.execute(&c_b, serde_json::json!({ "path": en_a.to_string_lossy() }))
                .await
                .is_err(),
            "B no puede leer A"
        );
    }

    /// Sin workspace en el contexto, el tool usa su root y su sandbox
    /// propios. Es lo de la invocacion directa por API y lo de todos los
    /// tests anteriores: el cambio no puede haber roto ese camino.
    #[tokio::test]
    async fn sin_workspace_en_el_contexto_manda_el_del_tool() {
        let dir = tempfile::TempDir::new().unwrap();
        let dentro = dir.path().join("dato.txt");
        std::fs::write(&dentro, "ok").unwrap();

        let abierto: Arc<RwLock<Box<dyn SandboxTrait>>> =
            Arc::new(RwLock::new(Box::new(ReadableOnly(vec![]))));
        let tool = ReadFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox: abierto,
        };

        // El directorio del tool siempre es legible por el short-circuit de
        // `resolve_under_workspace`, asi que el fallback tiene que andar.
        let leido = tool
            .execute(&ctx(), serde_json::json!({ "path": dentro.to_string_lossy() }))
            .await
            .expect("sin workspace en el contexto manda el root propio");
        assert!(leido.contains("ok"));
    }
}

#[cfg(test)]
mod sandbox_enabled_tests {
    //! `sandbox.enabled` tiene que significar algo.
    //!
    //! Antes el flag era inerte: los tools de filesystem piden
    //! `readable_paths_resolved()` / `writable_paths_resolved()` y nunca
    //! `is_readable()` / `is_writable()`, que son los unicos metodos donde
    //! el flag se leia. El doc del trait decia que `enabled == false`
    //! significa "sin restricciones", asi que la casilla del form mentia.
    //!
    //! Con el flag apagado el alcance pasa a ser el prefijo `UNRESTRICTED`
    //! (`/`), que matchea cualquier path absoluto. El `cwd` del shell y el
    //! root del workspace NO cambian: el agente sigue trabajando adentro del
    //! entorno, lo que cambia es a que puede llegar.

    use super::*;
    use crate::sandbox::SandboxConfig as SandboxTrait;
    use crate::tools::read::read_file::ReadFileTool;
    use crate::tools::write::write_file::WriteFileTool;
    use crate::tools::{ExecuteContext, Scope, Tool, WorkspaceScope};
    use std::path::{Path, PathBuf};

    struct Configurable {
        enabled: bool,
        readable: Vec<PathBuf>,
        writable: Vec<PathBuf>,
    }

    impl SandboxTrait for Configurable {
        fn enabled(&self) -> bool {
            self.enabled
        }
        fn is_writable(&self, p: &Path) -> bool {
            self.writable.iter().any(|r| p.starts_with(r))
        }
        fn is_readable(&self, p: &Path) -> bool {
            self.readable.iter().any(|r| p.starts_with(r))
        }
        fn readable_paths_resolved(&self, _ws: &Path) -> Vec<PathBuf> {
            self.readable.clone()
        }
        fn writable_paths_resolved(&self, _ws: &Path) -> Vec<PathBuf> {
            self.writable.clone()
        }
    }

    fn ctx() -> ExecuteContext {
        ExecuteContext {
            agent_id: "test".into(),
            workspace: None,
            cancel: None,
            http_client: None,
        }
    }

    fn arc(cfg: Configurable) -> Arc<tokio::sync::RwLock<Box<dyn SandboxTrait>>> {
        Arc::new(tokio::sync::RwLock::new(Box::new(cfg) as Box<dyn SandboxTrait>))
    }

    /// Apagado: `readable()` devuelve el prefijo de todo el sistema.
    #[tokio::test]
    async fn apagado_da_alcance_completo() {
        let celda = arc(Configurable { enabled: false, readable: vec![], writable: vec![] });
        let root = PathBuf::from("/srv/ws");
        let scope = Scope { root: &root, sandbox: &celda };

        assert!(!scope.enforced().await, "tiene que reportarse apagado");
        assert_eq!(
            scope.readable().await,
            vec![PathBuf::from(crate::tools::UNRESTRICTED)]
        );
        assert_eq!(
            scope.writable().await,
            vec![PathBuf::from(crate::tools::UNRESTRICTED)]
        );
    }

    /// Encendido: los limites siguen mandando.
    #[tokio::test]
    async fn encendido_sigue_limitando() {
        let celda = arc(Configurable {
            enabled: true,
            readable: vec![PathBuf::from("/srv/lectura")],
            writable: vec![],
        });
        let root = PathBuf::from("/srv/ws");
        let scope = Scope { root: &root, sandbox: &celda };

        assert!(scope.enforced().await);
        assert_eq!(scope.readable().await, vec![PathBuf::from("/srv/lectura")]);
        assert!(scope.writable().await.is_empty());
    }

    /// El efecto en una tool real: apagado lee fuera del root, encendido no.
    #[tokio::test]
    async fn apagando_el_sandbox_se_lee_fuera_del_root() {
        let ws = tempfile::TempDir::new().unwrap();
        let fuera = tempfile::TempDir::new().unwrap();
        let archivo = fuera.path().join("dato.txt");
        std::fs::write(&archivo, "secreto").unwrap();

        // Apagado: pasa.
        let apagado = arc(Configurable { enabled: false, readable: vec![], writable: vec![] });
        let tool_apagado = ReadFileTool {
            workspace_root: ws.path().to_path_buf(),
            sandbox: apagado,
        };
        assert!(
            tool_apagado
                .execute(&ctx(), serde_json::json!({ "path": archivo.to_string_lossy() }))
                .await
                .is_ok(),
            "con el sandbox apagado se lee fuera del root, como promete el doc del trait"
        );

        // Encendido y sin la ruta en la lista: se rechaza.
        let encendido = arc(Configurable { enabled: true, readable: vec![], writable: vec![] });
        let tool_encendido = ReadFileTool {
            workspace_root: ws.path().to_path_buf(),
            sandbox: encendido.clone(),
        };
        assert!(
            tool_encendido
                .execute(&ctx(), serde_json::json!({ "path": archivo.to_string_lossy() }))
                .await
                .is_err(),
            "con el sandbox encendido y la lista vacia, se rechaza"
        );
        let _ = &tool_encendido;
    }

    /// Apagado el sandbox, escribir tambien sale.
    #[tokio::test]
    async fn apagando_el_sandbox_se_escribe_fuera_del_root() {
        let ws = tempfile::TempDir::new().unwrap();
        let fuera = tempfile::TempDir::new().unwrap();
        let celda = arc(Configurable { enabled: false, readable: vec![], writable: vec![] });
        let tool = WriteFileTool {
            workspace_root: ws.path().to_path_buf(),
            sandbox: celda,
        };
        let destino = fuera.path().join("nuevo.txt");
        let r = tool
            .execute(
                &ctx(),
                serde_json::json!({ "command": "create", "path": destino.to_string_lossy(), "content": "x" }),
            )
            .await;
        assert!(r.is_ok(), "con el sandbox apagado escribe fuera del root: {r:?}");
        assert!(destino.exists());
    }

    /// Apagar el sandbox NO abre el traversal: `normalize_path` corre
    /// siempre, y un `..` que suba por encima de `/` sigue dando error.
    #[tokio::test]
    async fn apagar_el_sandbox_no_abre_el_traversal() {
        use crate::tools::helpers::resolve_under_workspace;
        let ws = PathBuf::from("/srv/ws");
        let r = resolve_under_workspace(&ws, "../../../etc/passwd", &[PathBuf::from("/")], false);
        assert!(r.is_err(), "el `..` por encima de la raiz tiene que seguir rechazandose");
    }

    /// El workspace en el contexto manda sobre el sandbox del tool.
    #[tokio::test]
    async fn el_workspace_apagado_manda_sobre_un_tool_encendido() {
        let ws = tempfile::TempDir::new().unwrap();
        let fuera = tempfile::TempDir::new().unwrap();
        let archivo = fuera.path().join("dato.txt");
        std::fs::write(&archivo, "secreto").unwrap();

        // El tool nace cerrado, pero la llamada viene con un workspace
        // apagado.
        let cerrado = arc(Configurable { enabled: true, readable: vec![], writable: vec![] });
        let tool = ReadFileTool {
            workspace_root: ws.path().to_path_buf(),
            sandbox: cerrado,
        };
        let apagado = arc(Configurable { enabled: false, readable: vec![], writable: vec![] });
        let c = ExecuteContext {
            agent_id: "t".into(),
            workspace: Some(WorkspaceScope {
                id: Some("w".into()),
                root: ws.path().to_path_buf(),
                sandbox: apagado,
            }),
            cancel: None,
            http_client: None,
        };
        assert!(
            tool.execute(&c, serde_json::json!({ "path": archivo.to_string_lossy() }))
                .await
                .is_ok()
        );
    }
}
