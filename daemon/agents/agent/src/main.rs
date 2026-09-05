//! agent
//!
//! default — LLM orchestrator with identity, memory, skills, mode detection,
//!
//! Many fields/functions in this crate are reserved for the v2
//! manifest work that was scaffolded but never fully wired up.
#![allow(dead_code)]

//! and tool filtering. Communicates via JSON-RPC 2.0 over stdin/stdout.
//!
//! CLI flags (EP-2026-08-15, multi-agent daemon):
//!   --id <name>          — agent id (default: $NEUROX_AGENT_ID env, falls back to "agent")
//!   --identity-dir <p>   — path to identity files (overrides $NEUROX_IDENTITY_DIR)
//!
//! Environment variables:
//!   `MINIMAX_API_KEY`     — required
//!   `MINIMAX_BASE_URL`    — default <https://api.minimaxi.chat/v1>
//!   `MINIMAX_MODEL`       — default MiniMax-M3
//!   `NEUROX_AGENT_ID`     — default "agent"
//!   `NEUROX_IDENTITY_DIR` — default ~/.local/share/neurox/identity/
//!
//! Methods (JSON-RPC 2.0 over stdin/stdout):
//!   - "process"       — main LLM call with text input (may return tool calls)
//!   - "tool_result"   — submit a tool execution result back to the LLM
//!   - "ping"          — health check (returns agent id + agent name)
//!   - "reset_session" — clear conversation history

// EP-0004 wave 5b: identity / llm / memory / mode / skills / tools_filter
// now live in `agent-lib`. We re-import them here for backwards
// compatibility with the rest of this file.
use agent_lib::identity as identity;
use agent_lib::llm as llm;
use agent_lib::memory as memory;
use agent_lib::mode as mode;
use agent_lib::skills as skills;
mod tools_filter {
    pub use agent_lib::tools_filter::*;
}

// EP-0004 wave 5b: shared agent logic now lives in `agent-lib`.
// The local `identity` / `llm` / `memory` / `mode` modules were moved
// verbatim into the crate.
use agent_lib::identity::{ActiveSkill, Fact};
use agent_lib::llm::{LlmClient, ToolCallRequest, ToolSpec};
use agent_lib::memory::{ChatMessage, WorkingMemory};
use agent_lib::mode::Mode;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

struct DefaultAgentState {
    client: Option<LlmClient>,
    identity: identity::Identity,
    facts: Vec<Fact>,
    skills: Vec<skills::Skill>,
    memory: WorkingMemory,
    all_tools: Vec<ToolSpec>,
    identity_dir: PathBuf,
    /// EP-2026-08-19: cached manifest so reset_session can reload facts
    /// from the same path the manifest specifies (instead of relying
    /// on the hardcoded `facts.yaml` fallback).
    manifest: Option<identity::Manifest>,
    /// EP-2026-08-15 (Fix 4): optional whitelist of tool names the
    /// subprocess is allowed to expose to the LLM. Injected by the
    /// daemon via `NEUROX_TOOLS_ALLOWLIST` (comma-separated). `None`
    /// means no restriction (admin / default behaviour).
    tools_allowlist: Option<Vec<String>>,
    /// Per-message model tracking. The daemon ships the selected
    /// provider+model in `process` params; if they differ from the
    /// currently-built `client`, the subprocess rebuilds the client
    /// before the LLM call. (`None` until the first such call.)
    current_provider_id: Option<String>,
    current_model: Option<String>,
}

/// EP-2026-08-15 (live-switch history injection): bulk-load messages
/// into the subprocess's WorkingMemory. Used by the daemon right after
/// `start_for_session` (which spawns a fresh subprocess with empty
/// memory) to seed the new agent with the session's prior history so
/// the LLM has context. Replaces the working memory contents — safe to
/// call multiple times (idempotent overwrite).
fn seed_history(memory: &mut WorkingMemory, msgs: Vec<memory::ChatMessage>) {
    memory.clear();
    for m in msgs {
        memory.add_raw_message(m);
    }
}

/// EP-2026-08-15: agent id resolved from CLI > env > "agent" default.
/// The id is exposed via the `ping` method so the daemon can verify
/// which subprocess it has at hand in the multi-agent setup.
fn resolve_agent_id() -> String {
    let from_cli: Option<String> = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == "--id")
        .map(|w| w[1].clone());
    from_cli
        .or_else(|| std::env::var("NEUROX_AGENT_ID").ok())
        .unwrap_or_else(|| "agent".to_string())
}

/// CLI args that override env defaults.
#[derive(Debug, Default)]
struct CliOverrides {
    identity_dir: Option<PathBuf>,
}

/// EP-2026-08-15: parse CLI flags `--identity-dir <path>`.
fn parse_cli_overrides() -> CliOverrides {
    let mut out = CliOverrides::default();
    let argv: Vec<String> = std::env::args().collect();
    for win in argv.windows(2) {
        if win[0] == "--identity-dir" {
            out.identity_dir = Some(PathBuf::from(&win[1]));
        }
    }
    out
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let agent_id = resolve_agent_id();

    // Resolve identity_dir + manifest BEFORE init the subscriber, so
    // we can apply observability.log_level from the manifest at startup
    // (highest priority). Falls back to RUST_LOG, then "info".
    let cli = parse_cli_overrides();
    let identity_dir = cli
        .identity_dir
        .or_else(|| std::env::var("NEUROX_IDENTITY_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("neurox/identity")
        });
    let manifest_pre = identity::load_manifest(&identity_dir, &agent_id);

    let cli_log_level = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == "--log-level")
        .map(|w| w[1].clone());

    let log_level = cli_log_level
        .or_else(|| std::env::var("RUST_LOG").ok())
        .or_else(|| manifest_pre.as_ref().map(|m| m.observability.log_level.clone()))
        .unwrap_or_else(|| "info".to_string());

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level)),
        )
        .with_writer(std::io::stderr)
        .init();

    eprintln!("[agent:{agent_id}] log_level={log_level}");

    // EP-2026-08-19: surface remaining v2 fields at startup so operators
    // can confirm what the manifest specifies (even if not yet
    // enforced at runtime).
    if let Some(m) = &manifest_pre {
        eprintln!(
            "[agent:{agent_id}] v2 manifest: harness={} skills_dir={} sampling_temp={:?} sampling_top_p={:?} sampling_max_tokens={:?} tools.policy={} mcp_servers={} memory.short_term={} memory.long_term.path={:?} memory.episodic.path={:?} memory.semantic.path={:?} guardrails.input={} guardrails.output={} output.type={} streaming.enabled={} budget.max_runtime_secs={:?} budget.cost_limit_usd={:?} stop.max_iterations={} stop.max_tool_calls={:?} sub_agents={} sandbox.network={} sandbox.process_spawn={} obs.log_level={} obs.trace={} obs.metrics={}",
            m.files.harness.len(),
            m.files.skills_dir,
            m.llm.sampling.temperature,
            m.llm.sampling.top_p,
            m.llm.sampling.max_tokens,
            m.tools.policy.r#type,
            m.mcp_servers.len(),
            m.memory.short_term.r#type,
            m.memory.long_term.path,
            m.memory.episodic.path,
            m.memory.semantic.path.as_deref().unwrap_or("(none)"),
            m.guardrails.input.len(),
            m.guardrails.output.len(),
            m.output.r#type,
            m.streaming.enabled,
            m.budget.max_runtime_secs,
            m.budget.cost_limit_usd,
            m.stop_conditions.max_iterations,
            m.stop_conditions.max_tool_calls,
            m.sub_agents.len(),
            m.permissions.sandbox.network_access,
            m.permissions.sandbox.process_spawn,
            m.observability.log_level,
            m.observability.trace,
            m.observability.metrics,
        );
    }

    let base_url = std::env::var("MINIMAX_BASE_URL")
        .unwrap_or_else(|_| "https://api.minimaxi.chat/v1".to_string());
    let model = std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M3".to_string());

    eprintln!(
        "[agent:{agent_id}] starting model={model} base_url={base_url} identity={}",
        identity_dir.display()
    );

    // Manifest (already pre-loaded above) — use as the single source
    // of truth for file paths, sampling, tools, memory, etc.
    let manifest = manifest_pre;
    let ident = identity::load_identity(&identity_dir, manifest.as_ref());
    let facts = identity::load_facts(&identity_dir, manifest.as_ref());
    eprintln!(
        "[default] loaded {} facts ({} active)",
        facts.len(),
        facts.iter().filter(|f| f.active).count()
    );

    // Load skills
    let skills_dir = identity_dir.join("skills");
    let skill_list = skills::load_skills(&skills_dir);

    // Init LLM client lazily — the agent must start even without MINIMAX_API_KEY
    // so that protocol-level tests (ping, unknown method) work in CI.
    let client = match std::env::var("MINIMAX_API_KEY") {
        Ok(api_key) => {
            let sampling = manifest
                .as_ref()
                .map(|m| llm::LlmSampling {
                    temperature: m.llm.sampling.temperature,
                    top_p: m.llm.sampling.top_p,
                    top_k: m.llm.sampling.top_k,
                    max_tokens: m.llm.sampling.max_tokens.map(|x| x as u32),
                    stop_sequences: m.llm.sampling.stop_sequences.clone(),
                    seed: m.llm.sampling.seed,
                    frequency_penalty: m.llm.sampling.frequency_penalty,
                    presence_penalty: m.llm.sampling.presence_penalty,
                })
                .unwrap_or_default();
            Some(LlmClient::with_sampling(api_key, base_url, model.clone(), sampling))
        }
        Err(_) => {
            eprintln!("[default] no MINIMAX_API_KEY, LLM calls will fail");
            None
        }
    };

    // EP-2026-08-19 (v2 manifest): expose stop_conditions.max_iterations
    // to the daemon via env var. The daemon's `max_tool_iterations()`
    // reads this at startup.
    if let Some(m) = &manifest {
        std::env::set_var(
            "NEUROX_MAX_TOOL_ITERATIONS",
            m.stop_conditions.max_iterations.to_string(),
        );
        eprintln!(
            "[default] stop.max_iterations={} from_manifest",
            m.stop_conditions.max_iterations
        );
    }

    // State
    // EP-2026-08-15 (Fix 4): parse the optional tools allowlist injected
    // by the daemon. Empty / unset var → no restriction (admin default).
    let tools_allowlist = std::env::var("NEUROX_TOOLS_ALLOWLIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|v: &Vec<String>| !v.is_empty());
    if let Some(ref allow) = tools_allowlist {
        eprintln!(
            "[agent:{agent_id}] tools_allowlist active: {} tool(s) restricted",
            allow.len()
        );
    }
    let state = Arc::new(Mutex::new(DefaultAgentState {
        client,
        identity: ident,
        facts,
        skills: skill_list,
        memory: WorkingMemory::new(),
        all_tools: Vec::new(),
        identity_dir,
        manifest,
        tools_allowlist,
        current_provider_id: None,
        current_model: None,
    }));

    // JSON-RPC loop
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let stdout: Arc<Mutex<tokio::io::Stdout>> = Arc::new(Mutex::new(tokio::io::stdout()));

    // EP-0003 Tier 3: graceful shutdown via SIGTERM/SIGINT. Spawn a
    // task that listens for both signals and notifies a `tokio::sync::Notify`
    // so the main loop can break cleanly. Without this, SIGTERM kills
    // the subprocess mid-iteration, losing any in-flight LLM stream
    // and the working memory snapshot.
    let shutdown = Arc::new(tokio::sync::Notify::new());
    {
        let shutdown = Arc::clone(&shutdown);
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut sigterm = match signal(SignalKind::terminate()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[default] failed to install SIGTERM handler: {e}");
                        return;
                    }
                };
                let mut sigint = match signal(SignalKind::interrupt()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[default] failed to install SIGINT handler: {e}");
                        return;
                    }
                };
                tokio::select! {
                    _ = sigterm.recv() => eprintln!(
                        "[default] SIGTERM received, finishing in-flight then exiting cleanly"
                    ),
                    _ = sigint.recv() => eprintln!(
                        "[default] SIGINT received, finishing in-flight then exiting cleanly"
                    ),
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
                eprintln!("[default] ctrl_c received, finishing in-flight then exiting cleanly");
            }
            // notify_one (not notify_waiters) — queues the notification
            // so the main loop's `shutdown.notified()` future fires even
            // if the signal arrives before the loop starts polling.
            // notify_waiters would only wake currently-waiting waiters;
            // we need persistence.
            shutdown.notify_one();
        });
    }

    let mut line = String::new();
    loop {
        line.clear();
        // Race stdin read vs shutdown signal. Either an EOF, an empty
        // line, or a signal ends the loop.
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                eprintln!("[default] shutdown complete");
                break;
            }
            read_res = reader.read_line(&mut line) => {
                let n = read_res?;
                if n == 0 {
                    break; // EOF
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let req: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(e) => {
                        write_error(&stdout, None, -32700, &format!("parse: {e}")).await?;
                        continue;
                    }
                };

                let id = req.get("id").cloned().unwrap_or(Value::Null);
                let method = req
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let params = req.get("params").cloned().unwrap_or(Value::Null);

                let result = handle_method(&method, params, state.clone(), stdout.clone()).await;

                let out = match result {
                    Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
                    Err(msg) => json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": {"code": -32603, "message": msg},
                    }),
                };
                {
                    let mut w = stdout.lock().await;
                    w.write_all(format!("{out}\n").as_bytes()).await?;
                    w.flush().await?;
                }
            }
        }
    }
    eprintln!("[default] exiting cleanly");
    // Force exit: tokio's signal-driver task holds the runtime open
    // even after our main future returns. std::process::exit bypasses
    // the runtime drop and exits immediately with code 0.
    std::process::exit(0);
}

/// EP-0003 Tier 3: per-request tracing span. Each top-level
/// `process` call gets its own span with `request_id` so the daemon
/// can correlate LLM / tool-call log lines back to a single user
/// request. `info!` lines emitted from inside the span inherit
/// `request_id` automatically via the `tracing` formatter's
/// `with_span_events` (configured by ops in `tracing_subscriber`).
fn request_span(request_id: &serde_json::Value, method: &str, sid: &str) -> tracing::Span {
    let id = match request_id {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "none".to_string(),
        _ => "unknown".to_string(),
    };
    tracing::info_span!(
        "process",
        request_id = %id,
        method = %method,
        session_id = %sid,
    )
}

async fn handle_method(
    method: &str,
    params: Value,
    state: Arc<Mutex<DefaultAgentState>>,
    stdout: Arc<Mutex<tokio::io::Stdout>>,
) -> Result<Value, String> {
    eprintln!("[default] method={method}");

    match method {
        "ping" => {
            let s = state.lock().await;
            Ok(json!({
                "pong": true,
                "agent_id": resolve_agent_id(),
                "model": s.client.as_ref().map(|c| c.model_name()).unwrap_or("not-configured"),
                "turns": s.memory.turn_count(),
                "tokens_est": s.memory.estimated_tokens(),
            }))
        }
        "reset_session" => {
            let mut s = state.lock().await;
            s.memory.clear();
            // Re-read facts on session reset (they might have been updated externally)
            s.facts = identity::load_facts(&s.identity_dir, s.manifest.as_ref());
            eprintln!("[default] session reset, re-loaded {} facts", s.facts.len());
            Ok(json!({"reset": true}))
        }
        "seed_history" => {
            // EP-2026-08-15 (live-switch history injection): the daemon
            // calls this once after start_for_session to populate the
            // subprocess's working memory with the session's prior
            // messages (read from its own DB). The new subprocess has
            // empty working memory, so without this it would have no
            // context of the previous conversation.
            //
            // JSON-RPC envelope wraps params inside params (see
            // `JsonRpcStdio::call`): the outer params has
            // { "session_id", "params": {...} }, so the inner
            // request body lives at params.params. We accept BOTH
            // shapes for resilience — flat { messages: [...] } (direct
            // invocation) and nested { params: { messages: [...] } }
            // (daemon call).
            let inner = params
                .get("params")
                .filter(|v| v.is_object())
                .unwrap_or(&params);
            let msgs = inner
                .get("messages")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| serde_json::from_value::<memory::ChatMessage>(v.clone()).ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let count = msgs.len();
            {
                let mut s = state.lock().await;
                seed_history(&mut s.memory, msgs);
            }
            eprintln!("[default] seed_history: {} messages loaded", count);
            Ok(json!({"seeded": count}))
        }
        "process" => handle_process(params, state, stdout).await,
        "tool_result" => handle_tool_result(params, state, stdout).await,
        other => Err(format!("unknown method: {other}")),
    }
}

async fn handle_process(
    params: Value,
    state: Arc<Mutex<DefaultAgentState>>,
    stdout: Arc<Mutex<tokio::io::Stdout>>,
) -> Result<Value, String> {
    // EP-2026-08-19 (v2 manifest): enforce budget.max_runtime_secs by
    // checking elapsed time before each LLM call. The cap is applied
    // per-handle_process invocation (i.e. per user message).
    let started_at = std::time::Instant::now();
    let max_runtime_secs = {
        let s = state.lock().await;
        s.manifest
            .as_ref()
            .and_then(|m| m.budget.max_runtime_secs)
    };
    if let Some(cap) = max_runtime_secs {
        if started_at.elapsed().as_secs() >= cap as u64 {
            return Err(format!("budget exceeded: max_runtime_secs={cap}"));
        }
    }

    // EP-0003 Tier 3: tracing span with the session_id so all
    // eprintln! / tracing! lines from this call inherit it. (We don't
    // have the JSON-RPC request id here — handle_method passes it
    // separately in a future commit; for now session_id is enough.)
    let sid_for_span = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let _enter = tracing::info_span!("process", session_id = %sid_for_span).entered();
    let text = params
        .get("params")
        .and_then(|p| p.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // EP-2026-08-19 (v2 manifest): apply input guardrails before
    // processing. Currently supports `max_length` (per-rule max_chars).
    // Other rule types (regex, no_secrets, blocklist) are parsed but
    // not yet enforced — log them so operators see what's configured.
    {
        let s = state.lock().await;
        if let Some(m) = &s.manifest {
            for rule in &m.guardrails.input {
                if let Some(max_chars) = rule.get("max_chars").and_then(|v| v.as_u64()) {
                    if text.len() as u64 > max_chars {
                        return Err(format!(
                            "guardrail: input exceeds max_chars ({max_chars})"
                        ));
                    }
                }
                // Other types: log only (not enforced yet).
                eprintln!("[default] guardrail.input rule: {rule}");
            }
        }
    }

    let sid = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    // Per-message LLM override: when the daemon ships `params.llm`
    // (kind + api_key + base_url + model), use it to (re)build the
    // LlmClient if it differs from the currently-bound one. Without
    // this the subprocess would always answer with whatever model it
    // was launched with — the user's mid-session model picker would be
    // purely cosmetic. The state already carries the manifest-derived
    // sampling so we re-apply it on rebuild.
    {
        let mut s = state.lock().await;
        if let Some(llm_cfg) = params.get("llm").cloned() {
            let kind_str = llm_cfg.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let api_key = llm_cfg.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
            let base_url = llm_cfg.get("base_url").and_then(|v| v.as_str()).unwrap_or("");
            let model = llm_cfg.get("model").and_then(|v| v.as_str()).unwrap_or("");
            let provider_id = params
                .get("provider_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let same = s.current_provider_id.as_deref() == Some(provider_id)
                && s.current_model.as_deref() == Some(model);
            if !api_key.is_empty()
                && !model.is_empty()
                && !kind_str.is_empty()
                && !same
            {
                if let Some(kind) =
                    tools_engine::backend::LlmProviderKind::from_str(kind_str)
                {
                    let sampling = s
                        .client
                        .as_ref()
                        .map(|c| c.sampling().clone())
                        .unwrap_or_default();
                    let new_client = LlmClient::with_kind(
                        api_key.to_string(),
                        base_url.to_string(),
                        model.to_string(),
                        kind,
                        sampling,
                    );
                    eprintln!(
                        "[default] switching LLM: provider={} model={} kind={}",
                        provider_id, model, kind_str
                    );
                    s.client = Some(new_client);
                    s.current_provider_id = Some(provider_id.to_string());
                    s.current_model = Some(model.to_string());
                } else {
                    eprintln!(
                        "[default] unknown LlmProviderKind '{kind_str}' in params.llm; keeping current client"
                    );
                }
            }
        }
    }

    // Parse tools sent by core
    let incoming_tools = llm::parse_tools_param(&params);

    // 1. Detect mode
    let detected_mode = mode::detect_mode(&text);
    eprintln!("[default] mode={:?} text_len={}", detected_mode, text.len());

    // 2. Match skills
    let (active_skills_info, _preferred_tools): (Vec<ActiveSkill>, Vec<String>) = {
        let s = state.lock().await;
        let matched = skills::match_skills(&s.skills, &text, 3);
        let info: Vec<ActiveSkill> = matched
            .iter()
            .map(|sk| ActiveSkill {
                name: sk.name.clone(),
                instructions: sk.instructions.clone(),
            })
            .collect();
        let prefs: Vec<String> = matched
            .iter()
            .flat_map(|sk| sk.preferred_tools.clone())
            .collect();
        if !matched.is_empty() {
            eprintln!(
                "[default] active skills: {}",
                matched
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        (info, prefs)
    };

    // 3. Filter tools
    let filtered_tools = {
        let mut s = state.lock().await;
        // Store all tools from core (update each time)
        if !incoming_tools.is_empty() {
            s.all_tools = incoming_tools;
        }
        tools_filter::filter_tools(
            &s.all_tools,
            &text,
            &detected_mode,
            s.tools_allowlist.as_deref(),
        )
    };

    // 4. Add user message to working memory
    {
        let mut s = state.lock().await;
        s.memory.add_message("user", Some(text.clone()));
    }

    // 5. Check compaction (skip if LLM client not configured)
    {
        let mut s = state.lock().await;
        if s.memory.needs_compaction() {
            if let Some(client) = s.client.as_ref() {
                eprintln!(
                    "[default] compaction needed: ~{} tokens",
                    s.memory.estimated_tokens()
                );
                let compaction_text = s.memory.get_compaction_text();
                let summary_messages = vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: Some(
                            "Resume la siguiente conversación en máximo 3 párrafos. Conserva: decisiones tomadas, hechos relevantes, y estado actual de la tarea. No incluyas saludos ni relleno.".to_string(),
                        ),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: Some(compaction_text),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                ];
                let summary = client.chat_utility(summary_messages, 150).await;
                if !summary.is_empty() {
                    s.memory.apply_compaction(summary);
                } else {
                    // EP-0003 Tier 3: surface compaction failure. The
                    // upstream call returned empty (LLM error, network
                    // error, auth issue, etc.) — without a summary the
                    // conversation will grow unbounded until the next
                    // retry succeeds. Log loudly + emit a
                    // `compaction_failed` JSON-RPC notification so the
                    // daemon's router picks it up and surfaces
                    // `Event::CompactionFailed` to the global event bus
                    // (which the dashboard can subscribe to).
                    let reason = "compaction call returned empty";
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "compaction_failed",
                        "params": { "reason": reason },
                    });
                    let mut w = stdout.lock().await;
                    let _ = w.write_all(format!("{notif}\n").as_bytes()).await;
                    let _ = w.flush().await;
                    tracing::error!(
                        session_id = %sid_for_span,
                        "[default] {reason}; \
                         conversation will continue to grow until next compaction retry"
                    );
                }
            } else {
                eprintln!("[default] compaction needed but MINIMAX_API_KEY not set — skipping");
            }
        }
    }

    // 6. Build system prompt and call LLM
    call_llm(
        state,
        stdout,
        &sid,
        &detected_mode,
        &active_skills_info,
        filtered_tools,
    )
    .await
}

async fn handle_tool_result(
    params: Value,
    state: Arc<Mutex<DefaultAgentState>>,
    stdout: Arc<Mutex<tokio::io::Stdout>>,
) -> Result<Value, String> {
    eprintln!(
        "[default] tool_result params keys: {:?}",
        params.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    let inner = params.get("params");
    eprintln!(
        "[default] tool_result inner: {:?}",
        inner.map(|v| v.to_string().chars().take(200).collect::<String>())
    );

    let call_id = params
        .get("params")
        .and_then(|p| p.get("call_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing call_id".to_string())?
        .to_string();

    // EP-0003 Tier 2: validate tool_call_id. The previous assistant
    // message in working memory has the tool_call_id(s) we just
    // emitted. The incoming tool_result's call_id must match one
    // of them (typically the first one). If not, the daemon (or a
    // buggy tool client) is sending a tool_result for a different
    // call — surface as a hard error rather than silently corrupting
    // the agent's working memory.
    {
        let s = state.lock().await;
        let last_assistant_call_ids: Vec<String> = s
            .memory
            .messages
            .iter()
            .rev()
            .find_map(|m| {
                if m.role == "assistant" {
                    m.tool_calls.as_ref().map(|calls| {
                        calls.iter().map(|c| c.id.clone()).collect::<Vec<_>>()
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();
        if !last_assistant_call_ids.is_empty()
            && !last_assistant_call_ids.iter().any(|id| id == &call_id)
        {
            return Err(format!(
                "tool_result call_id mismatch: got '{call_id}', \
                 expected one of {last_assistant_call_ids:?}"
            ));
        }
    }

    let result_text = params
        .get("params")
        .and_then(|p| p.get("result"))
        .map(|v| {
            // If it's a string, extract it directly; otherwise serialize as JSON
            v.as_str()
                .map(String::from)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_else(|| "null".to_string());

    eprintln!(
        "[default] tool_result call_id={} result_len={}",
        call_id,
        result_text.len()
    );

    let sid = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    // Add tool result message to memory
    {
        let mut s = state.lock().await;
        s.memory.add_raw_message(ChatMessage {
            role: "tool".to_string(),
            content: Some(result_text),
            tool_calls: None,
            tool_call_id: Some(call_id),
        });
    }

    // Re-call LLM with updated history (continue conversation)
    // EP-2026-08-15 (Fix 4): respect the allowlist even on tool_result
    // continuation. Without this, an agent in user/read-only mode could
    // bypass the restriction via a tool chain (LLM emits write_file in
    // turn 1, we get the result back, we hand it ALL tools in turn 2).
    let filtered_tools = {
        let s = state.lock().await;
        match s.tools_allowlist.as_deref() {
            Some(list) if !list.is_empty() => {
                let set: std::collections::HashSet<&str> =
                    list.iter().map(String::as_str).collect();
                s.all_tools
                    .iter()
                    .filter(|t| set.contains(t.function.name.as_str()))
                    .cloned()
                    .collect()
            }
            _ => s.all_tools.clone(), // On tool_result continuation, use all tools (Build-like)
        }
    };

    call_llm(
        state,
        stdout,
        &sid,
        &Mode::Build, // tool_result always gets full power (within allowlist)
        &[],
        filtered_tools,
    )
    .await
}

async fn call_llm(
    state: Arc<Mutex<DefaultAgentState>>,
    stdout: Arc<Mutex<tokio::io::Stdout>>,
    sid: &str,
    mode: &Mode,
    active_skills: &[ActiveSkill],
    tools: Vec<ToolSpec>,
) -> Result<Value, String> {
    // Build system prompt
    let (system_prompt, messages) = {
        let s = state.lock().await;
        let prompt = identity::build_system_prompt(
            &s.identity,
            &s.facts,
            &s.memory.context_summary,
            active_skills,
            mode.instruction(),
        );
        let msgs = s.memory.get_messages_for_llm();
        (prompt, msgs)
    };

    // Assemble full message list: system + conversation
    let mut full_messages = vec![ChatMessage {
        role: "system".to_string(),
        content: Some(system_prompt),
        tool_calls: None,
        tool_call_id: None,
    }];
    full_messages.extend(messages);

    // Call LLM streaming (returns a clear error if client not configured)
    let result = {
        let s = state.lock().await;
        match s.client.as_ref() {
            Some(client) => client.chat_stream(full_messages, tools, &stdout).await?,
            None => {
                return Err(
                    "MINIMAX_API_KEY not set — cannot call LLM. Set the env var and restart the agent."
                        .to_string(),
                );
            }
        }
    };

    // Emit thinking notification if present
    if let Some(ref thinking) = result.thinking {
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "thinking_delta",
            "params": { "text": thinking },
        });
        let mut w = stdout.lock().await;
        let _ = w.write_all(format!("{notif}\n").as_bytes()).await;
        let _ = w.flush().await;
    }

    // Handle tool calls vs final response
    if let Some(ref tool_calls) = result.tool_calls {
        // Save assistant message with tool calls to memory
        {
            let mut s = state.lock().await;
            s.memory.add_raw_message(ChatMessage {
                role: "assistant".to_string(),
                content: if result.text.is_empty() {
                    None
                } else {
                    Some(result.text.clone())
                },
                tool_calls: Some(
                    tool_calls
                        .iter()
                        .map(|tc| ToolCallRequest {
                            id: tc.id.clone(),
                            kind: "function".to_string(),
                            function: llm::ToolCallFunction {
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                            },
                        })
                        .collect(),
                ),
                tool_call_id: None,
            });
        }

        // Return tool_call to core (core dispatches and calls us back)
        let first = &tool_calls[0];
        // EP-0003 Tier 2: hard-fail on malformed JSON args. The previous
        // behavior (`unwrap_or(Value::Null)`) silently executed the tool
        // with null arguments, which masked LLM JSON syntax errors and
        // produced confusing downstream behavior (tools receiving
        // empty objects). Now: surface as a String error so the call
        // fails loud, the iteration loop terminates, and the user sees
        // a clear error.
        let args: Value = serde_json::from_str(&first.function.arguments).map_err(|e| {
            eprintln!(
                "[default] malformed tool_call arguments JSON: \
                 tool={} args={:?} err={e}",
                first.function.name,
                first.function.arguments
            );
            format!("malformed tool_call arguments JSON: {e}")
        })?;
        let model = state
            .lock()
            .await
            .client
            .as_ref()
            .map(|c| c.model_name().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(json!({
            "text": result.text,
            "model": model,
            "session_id": sid,
            "tool_call": {
                "id": first.id,
                "name": first.function.name,
                "args": args,
            },
            "tool_calls": tool_calls.iter().map(|t| {
                json!({
                    "id": t.id,
                    "name": t.function.name,
                    "args": serde_json::from_str::<Value>(&t.function.arguments).unwrap_or(Value::Null),
                })
            }).collect::<Vec<_>>(),
            "iterations": 1,
        }))
    } else {
        // Final text response — save to memory
        {
            let mut s = state.lock().await;
            s.memory.add_message("assistant", Some(result.text.clone()));
        }

        let s = state.lock().await;
        Ok(json!({
            "text": result.text,
            "model": s.client.as_ref().map(|c| c.model_name()).unwrap_or("unknown"),
            "session_id": sid,
            "iterations": 1,
            "tokens_out": result.tokens_out,
            "turns": s.memory.turn_count(),
            "streamed": true,
        }))
    }
}

async fn write_error(
    stdout: &Arc<Mutex<tokio::io::Stdout>>,
    id: Option<Value>,
    code: i32,
    message: &str,
) -> anyhow::Result<()> {
    let out = json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {"code": code, "message": message},
    });
    let mut w = stdout.lock().await;
    w.write_all(format!("{out}\n").as_bytes()).await?;
    w.flush().await?;
    Ok(())
}
