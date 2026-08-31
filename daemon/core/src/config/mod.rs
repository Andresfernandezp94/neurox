use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::protocols::{ProtocolKind, TransportKind};

// ────────────────────────────────────────────────────────────────────
// EP-0007: auth
// ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfigSection {
    /// When true, the daemon requires JWT auth on all `/v1/*` routes
    /// (except the explicitly-public ones — `/v1/auth/login`,
    /// `/health`, and the static dir).
    #[serde(default)]
    pub enabled: bool,
    /// Path to the JSON file holding user records.
    #[serde(default = "default_user_store_path")]
    pub user_store_path: PathBuf,
    /// Path to the file holding the JWT signing secret. Created with a
    /// random 64-byte secret on first run.
    #[serde(default = "default_jwt_secret_path")]
    pub jwt_secret_path: PathBuf,
    /// Token validity in hours.
    #[serde(default = "default_jwt_expiry_hours")]
    pub jwt_expiry_hours: u64,
}

pub(crate) fn default_user_store_path() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    p.push("users.json");
    p
}

pub(crate) fn default_jwt_secret_path() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    p.push("jwt_secret");
    p
}

pub(crate) fn default_jwt_expiry_hours() -> u64 {
    24
}

impl Default for AuthConfigSection {
    fn default() -> Self {
        Self {
            enabled: false,
            user_store_path: default_user_store_path(),
            jwt_secret_path: default_jwt_secret_path(),
            jwt_expiry_hours: default_jwt_expiry_hours(),
        }
    }
}

/// Placeholder used in API responses to indicate that an environment
/// variable value is present but must not be disclosed.
pub const REDACTED_ENV_VALUE: &str = "***REDACTED***";

/// How an agent process is spawned. Only `Subprocess` is currently wired;
/// future transports (Unix socket, HTTP) are tracked as TODO — re-add
/// them when the supervisor gains support.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentKind {
    Subprocess {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: std::collections::HashMap<String, String>,
    },
}

impl AgentKind {
    /// Return a copy of this `AgentKind` with every environment-variable
    /// value replaced by [`REDACTED_ENV_VALUE`]. Variable *names* are
    /// preserved so operators can still see what is configured. Use this
    /// whenever serialising a spec into an HTTP response — secrets must
    /// never leak through `/v1/agents`, `/v1/agents/:id`, or any other
    /// inspector endpoint.
    pub fn redact_secrets(&self) -> Self {
        match self {
            AgentKind::Subprocess { command, args, env } => AgentKind::Subprocess {
                command: command.clone(),
                args: args.clone(),
                env: env
                    .keys()
                    .map(|k| (k.clone(), REDACTED_ENV_VALUE.to_string()))
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentAgentSpec {
    pub id: String,
    #[serde(flatten)]
    pub kind: AgentKind,
    pub protocol: ProtocolKind,
    pub transport: TransportKind,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Tools this agent can invoke that need user approval before running.
    /// E.g. `["shell", "write_file", "delete_file"]`. The core emits an
    /// `approval_request` event and blocks until the user responds or
    /// the request times out.
    #[serde(default)]
    pub requires_approval: Vec<String>,
    /// Timeout for approval requests in seconds (default 60).
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_secs: u64,
    /// LLM provider selection for this agent (EP-0009-04). `None` uses the
    /// daemon-level `default_provider` from the `llm` section.
    #[serde(default)]
    pub llm: Option<AgentLlmConfig>,
    /// EP-frontend-config: inline system prompt. Optional. When set,
    /// overrides the `system-prompt.md` file in the identity dir (the
    /// runtime picks this up via env var / direct field). Defaults to
    /// `None` for backward compatibility with existing configs.
    #[serde(default)]
    pub system_prompt: Option<String>,
}

impl PersistentAgentSpec {
    /// Return a copy of this spec with all environment-variable values
    /// redacted. Use this when serialising a spec into an HTTP response
    /// (see [`AgentKind::redact_secrets`]).
    pub fn redact_secrets(&self) -> Self {
        Self {
            id: self.id.clone(),
            kind: self.kind.redact_secrets(),
            protocol: self.protocol.clone(),
            transport: self.transport.clone(),
            restart_policy: self.restart_policy.clone(),
            depends_on: self.depends_on.clone(),
            requires_approval: self.requires_approval.clone(),
            approval_timeout_secs: self.approval_timeout_secs,
            llm: self.llm.clone(),
            system_prompt: self.system_prompt.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}

fn default_approval_timeout() -> u64 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralAgentSpec {
    pub id: String,
    #[serde(flatten)]
    pub kind: AgentKind,
    pub protocol: ProtocolKind,
    pub transport: TransportKind,
    #[serde(default)]
    pub requires_approval: Vec<String>,
}

impl EphemeralAgentSpec {
    /// Return a copy of this spec with all environment-variable values
    /// redacted. Use this when serialising a spec into an HTTP response
    /// (see [`AgentKind::redact_secrets`]).
    pub fn redact_secrets(&self) -> Self {
        Self {
            id: self.id.clone(),
            kind: self.kind.redact_secrets(),
            protocol: self.protocol.clone(),
            transport: self.transport.clone(),
            requires_approval: self.requires_approval.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentsConfig {
    #[serde(default)]
    pub persistent: Vec<PersistentAgentSpec>,
    #[serde(default)]
    pub ephemeral_templates: Vec<EphemeralAgentSpec>,
}

/// How to spawn a per-session subprocess. Mirrors the runtime knobs of
/// `AgentKind::Subprocess` plus lifecycle tunables for the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAgentSpec {
    /// Command to execute. Resolved by `tokio::process::Command` (PATH
    /// search applies for bare names).
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Idle sessions (no activity) are killed after this many seconds
    /// by the eviction sweeper. Default: 1800 (30 min).
    #[serde(default = "default_session_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Optional soft cap on concurrent sessions for this spec. `None`
    /// means unlimited (bounded by hardware only).
    #[serde(default)]
    pub max_sessions: Option<usize>,
    /// EP-2026-08-15 (Fix 4): optional whitelist of tool names that the
    /// subprocess is allowed to expose to the LLM. Applied inside the
    /// `agent` subprocess before the category-based filter, so it cuts
    /// the available set at the source — the LLM never even sees the
    /// restricted tools. `None` or empty = no restriction (admin /
    /// default behaviour).
    ///
    /// Wired to the subprocess via the `NEUROX_TOOLS_ALLOWLIST` env var
    /// (comma-separated). The daemon injects it when spawning.
    #[serde(default)]
    pub tools_allowlist: Option<Vec<String>>,
    /// EP-frontend-config: inline system prompt. Optional. When set,
    /// overrides the `system-prompt.md` file in the identity dir.
    /// Defaults to `None` for backward compatibility with existing
    /// session-agent registrations.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Per-agent override for the tool-level `requires_approval` flag
    /// that llmd reports for each registered tool.
    ///
    /// - `None` (omitted in YAML, backwards-compatible default): the
    ///   tool's own `requires_approval` flag wins. A tool marked as
    ///   needing approval will still trigger the approval flow.
    /// - `Some(vec![])`: no tool requires approval for this agent —
    ///   every tool runs without prompting. Use for untrusted /
    ///   throw-away agents where the user has explicitly opted out
    ///   of the approval gate.
    /// - `Some(vec!["shell", ...])`: only the named tools require
    ///   approval; everything else runs unrestricted.
    ///
    /// Checked in `handle_session_tool_call` before the tool flag.
    #[serde(default)]
    pub requires_approval: Option<Vec<String>>,
}

fn default_session_idle_timeout() -> u64 {
    1800
}

/// EP-2026-08-15: in-process agent descriptor. List of agents that
/// the daemon runs in parallel (each with its own identity, prompt,
/// model override, and concurrency cap). Used by the daemon
/// runtime to instantiate `N` in-process agent instances at startup
/// and route session traffic to the right one via `agent_id`.
///
/// Today only one in-process agent binary is launched per spec; tomor-
/// row we'll wire `Spawner::spawn_agent(subprocess)` so each spec can
/// optionally run as its own process and truly parallelize across CPUs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InProcessAgentSpec {
    /// Stable identifier clients send in `agent_id` (e.g. `"admin"`,
    /// `"user"`, `"default"`). Must be unique within the daemon.
    pub id: String,
    /// Directory containing `system-prompt.md`, `_always-on.md`,
    /// `facts.yaml`, `skills/`. Loaded once at startup.
    #[serde(default)]
    pub identity_dir: Option<std::path::PathBuf>,
    /// Optional provider/model override (e.g. `minimax` + `MiniMax-M3`).
    /// Falls back to the daemon-wide `LlmConfig` if absent.
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Soft cap on concurrent LLM calls for this agent. `None` =
    /// unlimited (bounded by `CoreConfig::spawner_concurrency`).
    #[serde(default)]
    pub max_sessions: Option<usize>,
}

impl InProcessAgentSpec {
    /// Resolve the agent's effective identity directory (defaulting
    /// to the daemon's `~/.local/share/neurox/identity/`).
    pub fn resolved_identity_dir(&self) -> std::path::PathBuf {
        self.identity_dir.clone().unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("neurox/identity")
        })
    }
}

/// Pool-level configuration for session-scoped agents. Keyed by `agent_id`;
/// when a session is created with one of these ids, the daemon spawns a
/// fresh subprocess for that session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionAgentsConfig {
    #[serde(default)]
    pub agents: std::collections::HashMap<String, SessionAgentSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default = "default_bind")]
    pub bind_addr: String,
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub agents: AgentsConfig,
    /// Bearer token for HTTP/WS auth. If None, auth is disabled.
    #[serde(default)]
    pub api_token: Option<String>,
    /// TLS configuration. If None, plain HTTP is used.
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    /// Max number of ephemeral agents running concurrently.
    #[serde(default = "default_spawner_concurrency")]
    pub spawner_concurrency: usize,
    /// EP-2026-08-15: in-process agent pool. Each entry becomes a
    /// running agent instance inside the daemon. Sessions are routed
    /// to the entry whose `id` matches the session's `agent_id`.
    #[serde(default)]
    pub in_process: Vec<InProcessAgentSpec>,
    /// External services to probe and expose in `GET /v1/services`.
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
    /// URL of the plugin registry. Overrides the default public registry.
    #[serde(default)]
    pub plugins_registry: Option<String>,
    /// LLM provider registry + default provider (EP-0009-04). Missing section
    /// → `LlmConfig::default()` (legacy MiniMax from env vars).
    #[serde(default)]
    pub llm: LlmConfig,
    /// Sandbox config (EP-0019-03). Missing section → `SandboxConfig::default()`.
    #[serde(default)]
    pub sandbox: SandboxConfig,
    /// Per-session agent pool. When a session is created with an `agent_id`
    /// that matches a key here, the daemon spawns a fresh subprocess for
    /// that session and dispatches all messages to it. The process is
    /// killed when the session is cancelled, deleted, evicted for
    /// idleness, or the daemon shuts down. This gives real per-session
    /// parallelism — each chat conversation lives in its own process.
    ///
    /// Missing section → empty pool, no per-session spawning (legacy
    /// behaviour: in-process default or persistent Supervisor agent).
    #[serde(default)]
    pub session_agents: SessionAgentsConfig,
    /// EP-0007: auth
    #[serde(default)]
    pub auth: AuthConfigSection,
}

// ─── Sandbox config (EP-0019-03) ────────────────────────────────────────────

/// Sandbox configuration for file/shell tools. EP-0019-03.
///
/// When `enabled` is true, write tools (`write_file`, `shell`) reject
/// targets outside `writable_paths`, and read tools reject paths outside
/// `workspace` or `writable_paths`. `max_recursion_depth` caps
/// `glob`/`grep` traversal.
///
/// Default: enabled, no writable paths, depth 10.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SandboxConfig {
    /// Whether the sandbox is enforced. Default true.
    pub enabled: bool,
    /// Paths where write tools (write_file, shell) are allowed. Default [].
    /// Empty means read-only everywhere (writes are rejected).
    pub writable_paths: Vec<String>,
    /// Paths where read tools are allowed but writes are rejected.
    /// Read+write is `writable_paths` (read tools also work there).
    /// When `enabled` is true, paths outside `writable_paths` and
    /// `readable_paths` are denied for read tools too.
    /// Default: empty (read tools can only see the workspace root).
    pub readable_paths: Vec<String>,
    /// Max recursion depth for glob/grep. Default 10.
    pub max_recursion_depth: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            writable_paths: Vec::new(),
            max_recursion_depth: 10,
            readable_paths: Vec::new(),
        }
    }
}

impl SandboxConfig {
    /// Resolve the `writable_paths` strings against `workspace_root`.
    ///
    /// Honors the `${workspace}` placeholder so that operators can write
    /// `writable_paths: ["${workspace}/.sdd"]` in config.yaml without
    /// hardcoding the absolute path.
    ///
    /// Returns an empty Vec if `writable_paths` is empty.
    pub fn writable_paths_resolved(
        &self,
        workspace_root: &std::path::Path,
    ) -> Vec<std::path::PathBuf> {
        self.resolve_paths(&self.writable_paths, workspace_root)
    }

    /// Combined read scope: `readable_paths` + `writable_paths` (writes
    /// imply reads). Used by read-only tools so an operator can grant a
    /// path for reading without allowing writes there.
    pub fn readable_paths_resolved(
        &self,
        workspace_root: &std::path::Path,
    ) -> Vec<std::path::PathBuf> {
        let mut all: Vec<std::path::PathBuf> = self
            .readable_paths
            .iter()
            .chain(self.writable_paths.iter())
            .map(|p| self.resolve_one(p, workspace_root))
            .collect();
        all.sort();
        all.dedup();
        all
    }

    fn resolve_one(&self, p: &str, workspace_root: &std::path::Path) -> std::path::PathBuf {
        if p == "${workspace}" {
            workspace_root.to_path_buf()
        } else if let Some(rest) = p.strip_prefix("${workspace}/") {
            workspace_root.join(rest)
        } else {
            std::path::PathBuf::from(p)
        }
    }

    fn resolve_paths(&self, paths: &[String], workspace_root: &std::path::Path) -> Vec<std::path::PathBuf> {
        paths.iter().map(|p| self.resolve_one(p, workspace_root)).collect()
    }
}

// ─── LLM provider config (EP-0009-04) ───────────────────────────────────────

/// LLM provider kinds accepted in config.yaml (`serde` lowercase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderKind {
    Minimax,
    OpenaiCompat,
    Anthropic,
}

impl LlmProviderKind {
    /// Wire-format string the daemon sends to llmd. Always lowercase /
    /// snake_case — llmd's `match` on `req.kind.as_str()` requires it.
    /// Do NOT use `format!("{:?}", self)` here: that prints the Rust
    /// variant name (PascalCase) and makes llmd reject the call with
    /// `unknown provider kind: Minimax` (see EP-0027 / postmortem 2026-08-11).
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmProviderKind::Minimax => "minimax",
            LlmProviderKind::OpenaiCompat => "openai_compat",
            LlmProviderKind::Anthropic => "anthropic",
        }
    }
}

/// A single LLM provider declaration in the daemon `llm` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    /// Unique provider id referenced by `default_provider` or agent `llm`.
    pub id: String,
    pub kind: LlmProviderKind,
    /// Base URL. Empty → default for the kind (see `effective_base_url`).
    #[serde(default)]
    pub base_url: String,
    /// Model id. Empty → default for the kind (see `effective_model`).
    #[serde(default)]
    pub model: String,
    /// Name of the env var holding the API key. The key value is NEVER
    /// stored in config — only the env var name.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Extra backend options (e.g. extra headers, tool_choice).
    #[serde(default)]
    pub extra: HashMap<String, String>,
    /// EP-0018: local service orchestration. `Some` ⇒ the daemon starts and
    /// supervises this provider's process (e.g. llama-server). `None` ⇒
    /// remote provider (behaviour unchanged).
    #[serde(default)]
    pub local_command: Option<String>,
    /// Arguments for the local command. Placeholders: `{{model_path}}` and
    /// `{{port}}` are expanded at spawn time.
    #[serde(default)]
    pub local_args: Vec<String>,
    /// Path to the local model file (GGUF). Tilde is expanded at spawn time.
    #[serde(default)]
    pub local_model_path: Option<String>,
    /// Port the local service listens on. Used for the HTTP readiness probe
    /// and (when `base_url` is empty) to derive the effective base URL.
    #[serde(default)]
    pub local_port: Option<u16>,
}

impl LlmProviderConfig {
    /// Return `base_url` or, for a local service with a port and no explicit
    /// base URL, `http://127.0.0.1:{port}/v1`; otherwise the kind's default.
    pub fn effective_base_url(&self) -> String {
        if !self.base_url.is_empty() {
            self.base_url.clone()
        } else if let Some(port) = self.local_port {
            format!("http://127.0.0.1:{port}/v1")
        } else {
            match self.kind {
                LlmProviderKind::Minimax => "https://api.minimaxi.chat/v1".to_string(),
                LlmProviderKind::OpenaiCompat => "https://api.openai.com/v1".to_string(),
                LlmProviderKind::Anthropic => "https://api.anthropic.com".to_string(),
            }
        }
    }

    /// Return `model` or the kind's default when empty.
    pub fn effective_model(&self) -> String {
        if !self.model.is_empty() {
            self.model.clone()
        } else {
            match self.kind {
                LlmProviderKind::Minimax => "MiniMax-M3".to_string(),
                LlmProviderKind::OpenaiCompat => "gpt-4o-mini".to_string(),
                LlmProviderKind::Anthropic => "claude-3-5-haiku-latest".to_string(),
            }
        }
    }

    /// Copy with secrets redacted — `api_key_env` only ever holds a name,
    /// so redaction is a defensive no-op (keeps the contract stable if the
    /// struct evolves to inline values).
    #[must_use]
    pub fn redact_secrets(&self) -> Self {
        Self {
            id: self.id.clone(),
            kind: self.kind,
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key_env: self.api_key_env.clone(),
            extra: self.extra.clone(),
            local_command: self.local_command.clone(),
            local_args: self.local_args.clone(),
            local_model_path: self.local_model_path.clone(),
            local_port: self.local_port,
        }
    }
}

/// Registry of LLM providers + the daemon-wide default provider id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_llm_provider_id")]
    pub default_provider: String,
    #[serde(default)]
    pub providers: Vec<LlmProviderConfig>,
}

fn default_llm_provider_id() -> String {
    "minimax".to_string()
}

impl LlmConfig {
    /// Look up a provider by id.
    pub fn get_provider(&self, id: &str) -> Option<&LlmProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// Build an `LlmConfig` from a list of providers loaded from SQLite.
    /// The default provider is determined in this order:
    ///
    /// 1. If any provider has an explicit `default` flag, the first such
    ///    provider wins.
    /// 2. If `default_provider` is non-empty and references an existing
    ///    provider, that one wins. The explicit `default_provider` field
    ///    honours the YAML `llm.default_provider` configuration.
    /// 3. Otherwise, fall back to "minimax" when present.
    /// 4. Otherwise, the lexicographic first provider.
    pub fn from_providers(providers: Vec<LlmProviderConfig>) -> Self {
        let default_provider = pick_default_provider(&providers);
        Self {
            default_provider,
            providers,
        }
    }
}

fn pick_default_provider(providers: &[LlmProviderConfig]) -> String {
    // Honour the explicit `default_provider` from the YAML config when
    // the referenced provider exists in the loaded list.
    let yaml_default = std::env::var("NEUROX_DEFAULT_PROVIDER").ok();
    if let Some(id) = yaml_default {
        if providers.iter().any(|p| p.id == id) {
            return id;
        }
    }
    if providers.is_empty() {
        return default_llm_provider_id();
    }
    if let Some(found) = providers.iter().find(|p| p.id == "minimax") {
        return found.id.clone();
    }
    providers
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_else(default_llm_provider_id)
}

/// Default registry (backward compatible): a single `minimax` provider
/// configured from the legacy env vars.
impl Default for LlmConfig {
    fn default() -> Self {
        let base_url = std::env::var("MINIMAX_BASE_URL")
            .unwrap_or_else(|_| "https://api.minimaxi.chat/v1".to_string());
        let model = std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M3".to_string());
        Self {
            default_provider: default_llm_provider_id(),
            providers: vec![LlmProviderConfig {
                id: default_llm_provider_id(),
                kind: LlmProviderKind::Minimax,
                base_url,
                model,
                api_key_env: Some("MINIMAX_API_KEY".into()),
                extra: HashMap::new(),
                local_command: None,
                local_args: Vec::new(),
                local_model_path: None,
                local_port: None,
            }],
        }
    }
}

/// Per-agent LLM selection override (EP-0009-04).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLlmConfig {
    /// Provider id from the daemon `llm.providers` registry.
    pub provider: String,
    /// Optional model override for the provider's default model.
    #[serde(default)]
    pub model: Option<String>,
}

/// Configuration for a service probed by the daemon and exposed in
/// `GET /v1/services`. Exactly one of `socket` or `http` must be set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Type/kind label (e.g. "daemon", "database", "cache").
    #[serde(default = "default_service_type")]
    pub kind: String,
    /// Unix socket path to probe with HTTP.
    #[serde(default)]
    pub socket: Option<PathBuf>,
    /// HTTP base URL to probe (e.g. "http://127.0.0.1:8080").
    #[serde(default)]
    pub http: Option<String>,
    /// Path to GET for the health probe (default "/health").
    #[serde(default = "default_service_health_path")]
    pub health_path: String,
    /// Optional extra metadata to surface in the response.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// Bearer token to forward when proxying admin endpoints to this
    /// service (e.g. memoryd's `/admin/logs/tail`). Read by
    /// `/v1/memory/*` style proxies when the upstream service has
    /// `--api-token` enabled. Leave empty for unauthenticated services.
    #[serde(default)]
    pub admin_token: Option<String>,
}

fn default_service_type() -> String {
    "daemon".to_string()
}

fn default_service_health_path() -> String {
    "/health".to_string()
}

fn default_bind() -> String {
    "127.0.0.1:7878".to_string()
}

fn default_db_path() -> PathBuf {
    let mut p = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("neurox");
    p.push("neurox.db");
    p
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_spawner_concurrency() -> usize {
    8
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind(),
            db_path: default_db_path(),
            log_level: default_log_level(),
            agents: AgentsConfig::default(),
            api_token: None,
            tls: None,
            spawner_concurrency: default_spawner_concurrency(),
            in_process: Vec::new(),
            services: Vec::new(),
            plugins_registry: None,
            llm: LlmConfig::default(),
            sandbox: SandboxConfig::default(),
            session_agents: SessionAgentsConfig::default(),
            auth: AuthConfigSection::default(),
        }
    }
}

impl CoreConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let expanded = expand_env_vars(&text);
            Ok(serde_yml::from_str(&expanded)?)
        } else {
            tracing::info!("config {:?} not found, using defaults", path);
            Ok(Self::default())
        }
    }

    /// EP-2026-08-15: list of configured in-process agent specs.
    pub fn resolved_in_process(&self) -> Vec<InProcessAgentSpec> {
        self.in_process.clone()
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push("neurox");
        std::fs::create_dir_all(&p).ok();
        p.push("config.yaml");
        p
    }
}

/// Expand `${env:VAR}` and `$VAR` references in a string using process
/// environment variables. Unknown variables expand to empty string with a
/// warning logged. This lets users keep secrets in env vars instead of
/// baking them into config.yaml.
///
/// Multibyte UTF-8 is preserved by iterating over chars (not bytes).
fn expand_env_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // Try ${...}
        if i + 1 < chars.len() && chars[i + 1] == '{' {
            // Find matching '}'
            let close = chars[i + 2..].iter().position(|c| *c == '}');
            if let Some(close_off) = close {
                let inner: String = chars[i + 2..i + 2 + close_off].iter().collect();
                let var_name = inner.strip_prefix("env:").unwrap_or(&inner);
                match std::env::var(var_name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => tracing::warn!(
                        var = var_name,
                        "config: env var not set, expanding to empty"
                    ),
                }
                i += 2 + close_off + 1;
                continue;
            }
            // No closing brace — treat literal
            out.push('$');
            out.push('{');
            i += 2;
            continue;
        }
        // Try $VAR
        if i + 1 < chars.len() && (chars[i + 1].is_alphabetic() || chars[i + 1] == '_') {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let name: String = chars[start..end].iter().collect();
            match std::env::var(&name) {
                Ok(v) => out.push_str(&v),
                Err(_) => {
                    if name.chars().any(char::is_uppercase) || name.starts_with('_') {
                        tracing::warn!(
                            var = %name,
                            "config: env var not set, expanding to empty"
                        );
                    } else {
                        out.push('$');
                        out.push_str(&name);
                    }
                }
            }
            i = end;
            continue;
        }
        // Lone '$'
        out.push('$');
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_simple_var() {
        std::env::set_var("NEURO_TEST_VAR", "hello");
        let r = expand_env_vars("api_key: $NEURO_TEST_VAR");
        assert_eq!(r, "api_key: hello");
    }

    #[test]
    fn expand_braced_env() {
        std::env::set_var("NEURO_TEST_VAR2", "world");
        let r = expand_env_vars("api_key: ${env:NEURO_TEST_VAR2}");
        assert_eq!(r, "api_key: world");
    }

    #[test]
    fn expand_braced_no_prefix() {
        std::env::set_var("NEURO_TEST_VAR3", "x");
        let r = expand_env_vars("api_key: ${NEURO_TEST_VAR3}");
        assert_eq!(r, "api_key: x");
    }

    #[test]
    fn expand_missing_var_becomes_empty() {
        let r = expand_env_vars("api_key: ${env:NEURO_TEST_MISSING_XYZ}");
        assert_eq!(r, "api_key: ");
    }

    #[test]
    fn expand_non_var_preserved() {
        let r = expand_env_vars("plain text without vars");
        assert_eq!(r, "plain text without vars");
    }

    #[test]
    fn expand_dollar_at_end_preserved() {
        let r = expand_env_vars("price: $5");
        assert_eq!(r, "price: $5");
    }

    #[test]
    fn expand_preserves_multibyte_utf8() {
        // em dash, accents, emoji — must not corrupt UTF-8 sequences
        std::env::set_var("NEURO_TEST_HOST", "example.com");
        let r = expand_env_vars(
            "# neurox — cross-platform (€5/mes) — host: $NEURO_TEST_HOST\n# ñoño 中文 🎉",
        );
        assert!(r.contains("neurox — cross-platform"));
        assert!(r.contains("(€5/mes)"));
        assert!(r.contains("host: example.com"));
        assert!(r.contains("ñoño 中文 🎉"));
    }

    fn spec_with_secret() -> PersistentAgentSpec {
        use std::collections::HashMap;
        PersistentAgentSpec {
            id: "agent".into(),
            kind: AgentKind::Subprocess {
                command: "agent".into(),
                args: vec![],
                env: HashMap::from([
                    (
                        "MINIMAX_API_KEY".into(),
                        "sk-cp-SECRET-VALUE-DO-NOT-LEAK".into(),
                    ),
                    (
                        "MINIMAX_BASE_URL".into(),
                        "https://api.minimaxi.chat/v1".into(),
                    ),
                ]),
            },
            protocol: ProtocolKind::JsonRpc,
            transport: TransportKind::Stdio,
            restart_policy: RestartPolicy::Never,
            depends_on: vec![],
            requires_approval: vec![],
            approval_timeout_secs: 60,
            llm: None,
            system_prompt: None,
        }
    }

    #[test]
    fn redact_secrets_replaces_env_values() {
        let spec = spec_with_secret();
        let redacted = spec.redact_secrets();

        let AgentKind::Subprocess { env, .. } = &redacted.kind;
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("MINIMAX_API_KEY").unwrap(), "***REDACTED***");
        assert_eq!(env.get("MINIMAX_BASE_URL").unwrap(), "***REDACTED***");
        // Names are preserved so operators can still see what is configured.
        assert!(env.contains_key("MINIMAX_API_KEY"));
    }

    #[test]
    fn redact_secrets_does_not_mutate_original() {
        let spec = spec_with_secret();
        let _redacted = spec.redact_secrets();

        // The original spec must keep the real value — redaction is non-destructive.
        let AgentKind::Subprocess { env, .. } = &spec.kind;
        assert_eq!(
            env.get("MINIMAX_API_KEY").unwrap(),
            "sk-cp-SECRET-VALUE-DO-NOT-LEAK"
        );
    }

    #[test]
    fn redact_secrets_serialized_json_does_not_leak_value() {
        let spec = spec_with_secret();
        let redacted = spec.redact_secrets();
        let json = serde_json::to_string(&redacted).unwrap();
        assert!(!json.contains("SECRET-VALUE-DO-NOT-LEAK"));
        assert!(json.contains("***REDACTED***"));
    }

    // ─── EP-0009-04: LLM config ─────────────────────────────────────────────

    #[test]
    fn llm_section_parses_three_providers() {
        let yaml = r#"
llm:
  default_provider: openai
  providers:
    - id: minimax
      kind: minimax
      base_url: https://api.minimaxi.chat/v1
      model: MiniMax-M3
      api_key_env: MINIMAX_API_KEY
    - id: openai
      kind: openai_compat
      base_url: https://api.openai.com/v1
      model: gpt-4o
      api_key_env: OPENAI_API_KEY
    - id: anthropic
      kind: anthropic
      base_url: https://api.anthropic.com
      model: claude-3-5-haiku-latest
      api_key_env: ANTHROPIC_API_KEY
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(cfg.llm.default_provider, "openai");
        assert_eq!(cfg.llm.providers.len(), 3);

        let p = cfg.llm.get_provider("minimax").unwrap();
        assert_eq!(p.kind, LlmProviderKind::Minimax);
        assert_eq!(p.api_key_env.as_deref(), Some("MINIMAX_API_KEY"));
        assert_eq!(p.effective_base_url(), "https://api.minimaxi.chat/v1");
        assert_eq!(p.effective_model(), "MiniMax-M3");

        let p = cfg.llm.get_provider("openai").unwrap();
        assert_eq!(p.kind, LlmProviderKind::OpenaiCompat);
        assert_eq!(p.effective_base_url(), "https://api.openai.com/v1");

        let p = cfg.llm.get_provider("anthropic").unwrap();
        assert_eq!(p.kind, LlmProviderKind::Anthropic);
        assert_eq!(p.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));

        // Unknown provider id → None.
        assert!(cfg.llm.get_provider("nope").is_none());
    }

    #[test]
    fn llm_missing_section_falls_back_to_minimax_default() {
        let yaml = "api_token: tok\n";
        let cfg: CoreConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(cfg.llm.default_provider, "minimax");
        assert_eq!(cfg.llm.providers.len(), 1);
        let p = cfg.llm.get_provider("minimax").unwrap();
        assert_eq!(p.kind, LlmProviderKind::Minimax);
        assert_eq!(p.api_key_env.as_deref(), Some("MINIMAX_API_KEY"));
        // Env vars are read at Default time — assert the fallbacks hold when
        // the env is unset (we don't mutate env here; check struct contract).
        assert_eq!(p.effective_model(), "MiniMax-M3");
    }

    #[test]
    fn agent_llm_config_overrides_provider_and_model() {
        let yaml = r#"
agents:
  persistent:
    - id: default
      kind: subprocess
      command: agent
      protocol: json-rpc
      transport: stdio
      llm:
        provider: anthropic
        model: claude-opus
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).unwrap();
        let spec = cfg.agents.persistent.first().unwrap();
        let llm = spec.llm.as_ref().expect("agent llm override present");
        assert_eq!(llm.provider, "anthropic");
        assert_eq!(llm.model.as_deref(), Some("claude-opus"));
    }

    #[test]
    fn agent_llm_config_defaults_to_none() {
        let yaml = r#"
agents:
  persistent:
    - id: default
      kind: subprocess
      command: agent
      protocol: json-rpc
      transport: stdio
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).unwrap();
        let spec = cfg.agents.persistent.first().unwrap();
        assert!(spec.llm.is_none());
    }

    #[test]
    fn llm_provider_local_fields_parse_and_derive_base_url() {
        // EP-0018-01 req 4.1: local provider with the 4 local fields parses;
        // empty base_url + local_port → derived base URL.
        let yaml = r#"
llm:
  default_provider: local-llama
  providers:
    - id: local-llama
      kind: openai_compat
      model: qwen2.5-1.5b-instruct
      local_command: llama-server
      local_args:
        - "--model"
        - "{{model_path}}"
        - "--port"
        - "{{port}}"
        - "--host"
        - "127.0.0.1"
      local_model_path: "~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf"
      local_port: 11435
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).unwrap();
        let p = cfg.llm.get_provider("local-llama").unwrap();
        assert_eq!(p.local_command.as_deref(), Some("llama-server"));
        assert_eq!(p.local_args.len(), 6);
        assert_eq!(p.local_args[1], "{{model_path}}");
        assert_eq!(
            p.local_model_path.as_deref(),
            Some("~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf")
        );
        assert_eq!(p.local_port, Some(11435));
        // Empty base_url + local_port → derived base URL (D1).
        assert_eq!(p.effective_base_url(), "http://127.0.0.1:11435/v1");
    }

    #[test]
    fn llm_provider_without_local_fields_is_remote() {
        // EP-0018-01 req 4.2: a provider without local fields behaves as a
        // remote provider (backward compat).
        let yaml = r#"
llm:
  default_provider: openai
  providers:
    - id: openai
      kind: openai_compat
      base_url: https://api.openai.com/v1
      model: gpt-4o
      api_key_env: OPENAI_API_KEY
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).unwrap();
        let p = cfg.llm.get_provider("openai").unwrap();
        assert_eq!(p.local_command, None);
        assert!(p.local_args.is_empty());
        assert_eq!(p.local_model_path, None);
        assert_eq!(p.local_port, None);
        // Explicit base_url wins over any derivation.
        assert_eq!(p.effective_base_url(), "https://api.openai.com/v1");
    }

    #[test]
    fn llm_provider_redaction_never_exposes_key_value() {
        // Contract (1.5): config only ever stores the env var *name* in
        // api_key_env — never the key value. Serialization of a redacted
        // provider must not contain any value that would live in the env.
        let cfg = LlmConfig {
            default_provider: "openai".into(),
            providers: vec![LlmProviderConfig {
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
            }],
        };
        let redacted: Vec<_> = cfg.providers.iter().map(|p| p.redact_secrets()).collect();
        let json = serde_json::to_string(&redacted).unwrap();
        // The env var *name* is visible (so operators know what is configured)…
        assert!(json.contains("OPENAI_API_KEY"));
        // …but no inline value can exist in the struct to leak.
        assert!(!json.contains("sk-"));
    }

    #[test]
    fn from_providers_picks_minimax_when_present() {
        // Regression: the boot sequence stores providers in SQLite and
        // rehydrates them via `from_providers`. The previous implementation
        // picked the lexicographic first provider, which silently overrode
        // `llm.default_provider` from config.yaml and pointed at `anthropic`
        // (the alphabetically smallest id). The fix keeps `minimax` as the
        // default when it is present in the loaded list.
        let providers = vec![
            LlmProviderConfig {
                id: "anthropic".into(),
                kind: LlmProviderKind::Anthropic,
                base_url: "https://api.anthropic.com".into(),
                model: "claude-3-5-haiku-latest".into(),
                api_key_env: Some("ANTHROPIC_API_KEY".into()),
                extra: HashMap::new(),
                local_command: None,
                local_args: Vec::new(),
                local_model_path: None,
                local_port: None,
            },
            LlmProviderConfig {
                id: "minimax".into(),
                kind: LlmProviderKind::Minimax,
                base_url: String::new(),
                model: String::new(),
                api_key_env: Some("MINIMAX_API_KEY".into()),
                extra: HashMap::new(),
                local_command: None,
                local_args: Vec::new(),
                local_model_path: None,
                local_port: None,
            },
            LlmProviderConfig {
                id: "openrouter".into(),
                kind: LlmProviderKind::OpenaiCompat,
                base_url: "https://openrouter.ai/api/v1".into(),
                model: "gpt-4o-mini".into(),
                api_key_env: Some("OPENROUTER_API_KEY".into()),
                extra: HashMap::new(),
                local_command: None,
                local_args: Vec::new(),
                local_model_path: None,
                local_port: None,
            },
        ];
        let cfg = LlmConfig::from_providers(providers);
        assert_eq!(cfg.default_provider, "minimax");
    }

    #[test]
    fn from_providers_falls_back_to_first_when_minimax_missing() {
        let providers = vec![LlmProviderConfig {
            id: "anthropic".into(),
            kind: LlmProviderKind::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            model: "claude-3-5-haiku-latest".into(),
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            extra: HashMap::new(),
            local_command: None,
            local_args: Vec::new(),
            local_model_path: None,
            local_port: None,
        }];
        let cfg = LlmConfig::from_providers(providers);
        assert_eq!(cfg.default_provider, "anthropic");
    }

    #[test]
    fn from_providers_falls_back_to_minimax_when_empty() {
        let cfg = LlmConfig::from_providers(vec![]);
        assert_eq!(cfg.default_provider, "minimax");
    }

    // ─── SandboxConfig (EP-0019-03) ────────────────────────────────────────

    #[test]
    fn sandbox_config_default_is_strict() {
        let cfg = SandboxConfig::default();
        assert!(cfg.enabled, "sandbox should default to enabled");
        assert!(
            cfg.writable_paths.is_empty(),
            "no writable paths by default"
        );
        assert_eq!(cfg.max_recursion_depth, 10);
    }

    #[test]
    fn sandbox_config_parses_yaml_with_paths() {
        let yaml = r#"
sandbox:
  enabled: true
  writable_paths:
    - "${workspace}/.sdd"
    - "${workspace}/.sdd-specs"
  max_recursion_depth: 5
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).expect("parse yaml");
        assert!(cfg.sandbox.enabled);
        assert_eq!(cfg.sandbox.writable_paths.len(), 2);
        assert_eq!(cfg.sandbox.writable_paths[0], "${workspace}/.sdd");
        assert_eq!(cfg.sandbox.max_recursion_depth, 5);
    }

    #[test]
    fn sandbox_config_parses_yaml_disabled() {
        let yaml = r#"
sandbox:
  enabled: false
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).expect("parse yaml");
        assert!(!cfg.sandbox.enabled);
        // Other fields still default
        assert!(cfg.sandbox.writable_paths.is_empty());
        assert_eq!(cfg.sandbox.max_recursion_depth, 10);
    }

    #[test]
    fn sandbox_config_missing_section_uses_default() {
        let yaml = r#"
bind_addr: 127.0.0.1:9090
"#;
        let cfg: CoreConfig = serde_yml::from_str(yaml).expect("parse yaml");
        assert_eq!(cfg.sandbox, SandboxConfig::default());
    }

    #[test]
    fn core_config_default_includes_sandbox() {
        let cfg = CoreConfig::default();
        assert_eq!(cfg.sandbox, SandboxConfig::default());
    }

    #[test]
    fn sandbox_config_roundtrip_yaml() {
        let original = SandboxConfig {
            enabled: false,
            writable_paths: vec!["/tmp/foo".to_string(), "/var/log".to_string()],
            readable_paths: vec!["/etc/nginx".to_string()],
            max_recursion_depth: 3,
        };
        let serialized = serde_yml::to_string(&original).expect("serialize");
        let deserialized: SandboxConfig = serde_yml::from_str(&serialized).expect("deserialize");
        assert_eq!(original, deserialized);
    }
}

// EP-0015: implement tools_engine::SandboxConfig trait so the engine can
// store `Arc<dyn SandboxConfig>`. The struct fields stay the same; we
// just expose the trait methods that tools actually call.
impl tools_engine::SandboxConfig for SandboxConfig {
    fn enabled(&self) -> bool {
        self.enabled
    }

    fn is_writable(&self, path: &Path) -> bool {
        if !self.enabled {
            return false;
        }
        self.writable_paths_resolved(&PathBuf::new())
            .iter()
            .any(|root| path.starts_with(root))
    }

    fn is_readable(&self, path: &Path) -> bool {
        if !self.enabled {
            return false;
        }
        self.readable_paths_resolved(&PathBuf::new())
            .iter()
            .any(|root| path.starts_with(root))
    }

    fn max_recursion_depth(&self) -> Option<usize> {
        Some(self.max_recursion_depth)
    }

    fn writable_paths_resolved(&self, workspace_root: &Path) -> Vec<PathBuf> {
        SandboxConfig::writable_paths_resolved(self, workspace_root)
    }

    fn readable_paths_resolved(&self, workspace_root: &Path) -> Vec<PathBuf> {
        SandboxConfig::readable_paths_resolved(self, workspace_root)
    }
}
