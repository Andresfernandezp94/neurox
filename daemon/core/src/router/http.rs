use axum::extract::{Extension, Path, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::sync::{Arc, OnceLock};
use std::path::PathBuf;
use std::path::Path as FsPath;
use tracing::info;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::auth::UserContext;
use crate::config::{EphemeralAgentSpec, PersistentAgentSpec, SessionAgentSpec};
use crate::events::Event;
use crate::plugins::PluginStatus;
use crate::router::AppState;

/// Lazy process start timestamp. Set on the first call to `health()`.
/// render the daemon's uptime without polling a separate endpoint.
static STARTED_AT: OnceLock<DateTime<Utc>> = OnceLock::new();

pub async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let started = STARTED_AT.get_or_init(Utc::now);
    let uptime_seconds = (Utc::now() - *started).num_seconds().max(0);
    // EP-0023-01 + EP-0023-02: tell the admin whether the daemon requires
    // auth so the LoginScreen gate can decide to render or skip.
    // EP-0007: also true when the new JWT-based auth is enabled.
    let auth_required = state.auth.auth.is_some();

    // Snapshot the connected plugins once — they're shared across all
    // sessions (tool proxy is daemon-wide).
    let mcp_plugins: Vec<serde_json::Value> = state
        .lifecycle
        .plugin_registry
        .list()
        .into_iter()
        .filter(|p| matches!(p.status, PluginStatus::Connected))
        .map(|p| {
            json!({
                "name": p.name,
                "status": p.status,
                "tools": p.tools,
                "skills": p.skills,
            })
        })
        .collect();

    // Per-session detail: gather pid/process + model from SQLite +
    // message count + plugin/MCP visibility.
    let running = state.lifecycle.session_agents.list().await;
    let mut details: Vec<serde_json::Value> = Vec::with_capacity(running.len());
    for a in running {
        let sid = a.session_id;
        let model = state.lifecycle.session.get_model(sid).await.ok().flatten();
        let messages = state.lifecycle.session.get_messages(sid).await.unwrap_or_default();
        let message_count = messages.len();
        let tokens_used = state.lifecycle.session.get_tokens_used(sid).await.ok().flatten();
        let inflight = a.inflight;
        details.push(json!({
            "session_id": sid,
            "agent_id": a.agent_id,
            // True only while the agent is actually processing a
            // request for this session (inflight counter > 0). Goes
            // false the moment the agent's response arrives.
            "active": inflight > 0,
            "inflight": inflight,
            "idle_secs": a.idle_secs,
            "uptime_secs": a.uptime_secs,
            "max_idle_secs": a.max_idle_secs,
            "pid": a.pid,
            "command": a.command,
            "args": a.args,
            "model": model.map(|(p, m)| json!({"provider_id": p, "model": m})),
            "context": {
                "messages": message_count,
                "tokens_used": tokens_used,
            },
            "mcp": mcp_plugins,
        }));
    }

    Json(json!({
        "status": "ok",
        "service": "neurox",
        "version": env!("CARGO_PKG_VERSION"),
        "started_at": started.to_rfc3339(),
        "uptime_seconds": uptime_seconds,
        "auth_required": auth_required,
        "sessions": {
            "running": details.len(),
            "specs": state.lifecycle.session_agents.list_specs().await.len(),
            "details": details,
        },
    }))
}

/// GET /v1/default/status — real-time context, provider, model, compaction state.
/// F4.4 mini: proxy to llmd's /v1/status.
pub async fn default_agent_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // EP-2026-08-19: previously proxied to llmd. Now reads engine
    // status directly; the engine is in-process so "unreachable"
    // can't happen in normal operation.
    Json(state.engine.status().await)
}

/// EP-0013 T-002: GET /livez — liveness probe. Always 200 if the
/// process is responsive. Used by K8s/systemd to decide if to restart.
pub async fn livez() -> StatusCode {
    StatusCode::OK
}

/// EP-0013 T-002: GET /readyz — readiness probe. 200 if all deps are
/// healthy (SQLite, engine, plugin registry), 503 with JSON if any is
/// degraded. Used by load balancers to decide if to send traffic.
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    let mut degraded: Vec<&str> = Vec::new();

    // SQLite check
    if let Err(e) = sqlx::query("SELECT 1").execute(&state.engine.db).await {
        tracing::warn!(error = %e, "readyz: SQLite query failed");
        degraded.push("sqlite");
    }

    // Engine check (the engine is in-process so this should always pass
    // unless the supervisor is in a bad state).
    let engine_status = state.engine.status().await;
    if engine_status.get("status").and_then(|v| v.as_str()) != Some("running") {
        degraded.push("engine");
    }

    if degraded.is_empty() {
        (StatusCode::OK, Json(json!({"ready": true}))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ready": false,
                "degraded": degraded,
            })),
        )
            .into_response()
    }
}

/// GET /v1/skills — list all loaded skills. F4.4c: proxy to llmd, then
/// merge in the operator's enable/disable overrides so the frontend
/// can render toggle rows in one round-trip.
pub async fn list_skills_endpoint(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut v = state.engine.list_skills();
    {
            // Best-effort merge: extract the `skills` array, attach an
            // `enabled` field per entry, and sync the registry so
            // operator overrides survive even for skills the operator
            // toggled before llmd reported them.
            if let Some(arr) = v.get_mut("skills").and_then(|s| s.as_array_mut()) {
                let names: Vec<String> = arr
                    .iter()
                    .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect();
                state.lifecycle.skills.sync_from_names(names.iter().map(String::as_str), true);
                for skill in arr.iter_mut() {
                    if let Some(name) = skill.get("name").and_then(|n| n.as_str()) {
                        let enabled = state.lifecycle.skills.is_enabled(name, true);
                        if let Some(obj) = skill.as_object_mut() {
                            obj.insert("enabled".to_string(), serde_json::Value::Bool(enabled));
                        }
                    }
                }
            }
            // Also surface the full operator override list so the UI
            // can render stale / unknown entries if needed.
            let overrides: Vec<serde_json::Value> = state
                .lifecycle
                .skills
                .list()
                .into_iter()
                .map(|s| json!({"name": s.name, "enabled": s.enabled}))
                .collect();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("overrides".to_string(), serde_json::Value::Array(overrides));
            }
        }
    Json(v)
}

/// Returns the list of external services the daemon probes, with their
/// live status, latency and version. Each service is probed on every
pub async fn list_services(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let started = STARTED_AT.get_or_init(Utc::now);
    let now = chrono::Utc::now();
    let now_ts = now.to_rfc3339();

    let mut services: Vec<serde_json::Value> = Vec::new();

    // 1) The daemon itself — always ok.
    services.push(json!({
        "id": "neurox",
        "name": "Daemon",
        "description": "Local agent orchestrator with WS events and HTTP API",
        "kind": "daemon",
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoint": format!("http://{}", state.config.bind_addr),
        "latency_ms": 0,
        "uptime_seconds": (now - *started).num_seconds().max(0),
        "started_at": started.to_rfc3339(),
        "last_checked_at": now_ts,
        "metadata": {
            "runtime": "tokio",
            "api": "rest+ws",
        },
    }));

    // 2) Configured external services — probe live.
    for svc in &state.config.services {
        let probe = probe_service(svc).await;
        let mut entry = json!({
            "id": svc.id,
            "name": svc.name,
            "description": svc.description,
            "kind": svc.kind,
            "status": probe.status,
            "version": probe.version,
            "endpoint": probe.endpoint,
            "latency_ms": probe.latency_ms,
            "uptime_seconds": probe.uptime_seconds,
            "last_checked_at": now_ts,
            "metadata": svc.metadata,
        });
        if let Some(msg) = probe.error {
            entry["error"] = json!(msg);
        }
        services.push(entry);
    }

    // 3) Connected clients (HTTP + WS).
    let clients: Vec<serde_json::Value> = state
        .events.clients
        .snapshot()
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id,
                "kind": c.kind,
                "detected_as": c.detected_as,
                "path": c.path,
                "method": c.method,
                "source_addr": c.source_addr,
                "user_agent": c.user_agent,
                "connected_at": c.connected_at,
                "duration_seconds": c.duration_seconds,
                "requests_count": c.requests_count,
            })
        })
        .collect();

    Json(json!({
        "services": services,
        "clients": clients,
        "server_time": now_ts,
    }))
}

#[derive(Debug)]
struct ProbeResult {
    status: &'static str,
    version: String,
    endpoint: String,
    latency_ms: u64,
    uptime_seconds: Option<i64>,
    error: Option<String>,
}

/// Probe a single service. Tries unix socket first, then HTTP. Returns
/// the structured result with status, latency, version (parsed from the
/// JSON body if present).
async fn probe_service(svc: &crate::config::ServiceConfig) -> ProbeResult {
    let endpoint = if let Some(s) = &svc.socket {
        format!("unix://{}", s.display())
    } else if let Some(h) = &svc.http {
        h.clone()
    } else {
        return ProbeResult {
            status: "error",
            version: "unknown".into(),
            endpoint: "".into(),
            latency_ms: 0,
            uptime_seconds: None,
            error: Some("service has neither socket nor http configured".into()),
        };
    };

    let health_path = if svc.health_path.starts_with('/') {
        svc.health_path.clone()
    } else {
        format!("/{}", svc.health_path)
    };

    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(2);

    let probe_result = if let Some(socket_path) = &svc.socket {
        probe_unix_socket(socket_path, &health_path, timeout).await
    } else if let Some(http_url) = &svc.http {
        probe_http(http_url, &health_path, timeout).await
    } else {
        Err("no transport".to_string())
    };

    let latency_ms = started.elapsed().as_millis() as u64;

    match probe_result {
        Ok((status, version, body)) => {
            let uptime = body
                .get("uptime_seconds")
                .and_then(|v| v.as_i64())
                .or_else(|| body.get("uptime").and_then(|v| v.as_i64()));
            ProbeResult {
                status,
                version,
                endpoint,
                latency_ms,
                uptime_seconds: uptime,
                error: None,
            }
        }
        Err(e) => ProbeResult {
            status: "unreachable",
            version: "unknown".into(),
            endpoint,
            latency_ms,
            uptime_seconds: None,
            error: Some(e),
        },
    }
}

async fn probe_unix_socket(
    path: &std::path::Path,
    health_path: &str,
    timeout: std::time::Duration,
) -> Result<(&'static str, String, serde_json::Value), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let stream = tokio::net::UnixStream::connect(path)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let buf = tokio::time::timeout(timeout, async move {
        let (mut read, mut write) = stream.into_split();
        let req =
            format!("GET {health_path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        write
            .write_all(req.as_bytes())
            .await
            .map_err(|e| format!("write: {e}"))?;
        write
            .shutdown()
            .await
            .map_err(|e| format!("shutdown: {e}"))?;
        let mut buf = Vec::new();
        read.read_to_end(&mut buf)
            .await
            .map_err(|e| format!("read: {e}"))?;
        Ok::<_, String>(buf)
    })
    .await
    .map_err(|_| "timeout".to_string())??;
    parse_http_response(&buf)
}

async fn probe_http(
    base: &str,
    health_path: &str,
    timeout: std::time::Duration,
) -> Result<(&'static str, String, serde_json::Value), String> {
    let url = format!("{}{}", base.trim_end_matches('/'), health_path);
    let resp = tokio::time::timeout(timeout, reqwest::get(&url))
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(|e| format!("http: {e}"))?;
    let status_code = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("body: {e}"))?;
    parse_http_response_from_body(status_code, &body)
}

/// Minimal HTTP response parser. Returns (status, version, parsed_json_body).
/// Accepts both 1.1 and 1.0 responses. For non-2xx, status="degraded" or "error".
fn parse_http_response(raw: &[u8]) -> Result<(&'static str, String, serde_json::Value), String> {
    let text = String::from_utf8_lossy(raw);
    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().ok_or("malformed response")?;
    let body = parts.next().unwrap_or("");
    let status_line = head.lines().next().ok_or("no status line")?;
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or("invalid status code")?;
    parse_http_response_from_body(status_code, body)
}

fn parse_http_response_from_body(
    status_code: u16,
    body: &str,
) -> Result<(&'static str, String, serde_json::Value), String> {
    let status = if (200..300).contains(&status_code) {
        "ok"
    } else if status_code >= 500 {
        "error"
    } else {
        "degraded"
    };
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::json!({}));
    let version = parsed
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    Ok((status, version, parsed))
}

pub async fn list_agents(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let persistent: Vec<_> = state
        .lifecycle.registry
        .list_persistent()
        .await
        .into_iter()
        .map(|s| s.redact_secrets())
        .collect();
    let ephemeral: Vec<_> = state
        .lifecycle.registry
        .list_ephemeral_templates()
        .await
        .into_iter()
        .map(|s| s.redact_secrets())
        .collect();
    let running = state.lifecycle.supervisor.list().await; // supervisor.list() already redacts

    // EP-2026-08-15: ONLY surface session_agents. The legacy
    // `in_process` slot (single long-lived agent subprocess) is
    // gone. Every agent here will get a dedicated per-session
    // subprocess on demand.
    let specs = state.lifecycle.session_agents.list_specs().await;
    let in_process: Vec<serde_json::Value> = specs
        .into_iter()
        .map(|(id, spec)| {
            json!({
                "type": "in_process",
                "id": id,
                "status": "ready",
                "kind": "session-isolated",
                "command": spec.command,
                "args": spec.args,
                "idle_timeout_secs": spec.idle_timeout_secs,
                "max_sessions": spec.max_sessions,
            })
        })
        .collect();
    // EP-2026-08-19: NO legacy `agent`/`default` injection here.
    // Every agent listed above comes from `session_agents` (the only
    // source of truth for spawnable agents). Clients that send
    // agent_id values not in this list will get a routing error.
    //
    // EP-2026-08-19: media tools in llmd now use a flat
    // `$HOME/neurox/{kind}/` path (no agent subdir). Sending an
    // `agent_id` for a media tool is harmless (the daemon forwards it
    // for routing, but llmd ignores it for the output dir).

    Json(json!({
        "persistent": persistent,
        "ephemeral_templates": ephemeral,
        "running": running,
        "in_process": in_process,
    }))
}

// EP-2026-08-15: runtime registration of a session_agent spec. Body
// specifies the agent's id, identity_dir (where the agent loads its
// system prompt), and the path to the agent binary. If the
// identity_dir doesn't exist or is empty, we materialize the
// template structure (system-prompt.md + _always-on.md + skills/)
// so a fresh agent has sensible defaults that the user can edit.
#[derive(Debug, Deserialize)]
pub struct RegisterInProcessAgentReq {
    pub id: String,
    pub identity_dir: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub idle_timeout_secs: Option<u64>,
    pub max_sessions: Option<usize>,
}

/// Materialize the directory structure for a freshly registered agent.
/// Creates `system-prompt.md`, `_always-on.md`, `facts.yaml`, and
/// `skills/` if they don't exist, with sensible starter content
/// referencing the agent id. Idempotent: existing files are not
/// overwritten.
async fn materialize_agent_template(id: &str, identity_dir: &FsPath) -> std::io::Result<()> {
    use std::fs;
    fs::create_dir_all(identity_dir)?;
    fs::create_dir_all(identity_dir.join("skills"))?;

    let system_prompt = format!(
        "# {id} — agent identity\n\n\
         You are the **{id}** agent of the neurox ecosystem. Your role is configured\n\
         by the user (operator) after registration. Personalize this file with the\n\
         behavior, capabilities, and limits you want this agent to have.\n\n\
         Identity directory: {}\n",
        identity_dir.display()
    );
    let system_prompt_path = identity_dir.join("system-prompt.md");
    if !system_prompt_path.exists() {
        fs::write(&system_prompt_path, system_prompt)?;
    }

    let always_on = "# Always-on rules\n\n\
         - Use the user's preferred language (Spanish if not detected otherwise).\n\
         - Be concise. Show your reasoning only when the task is non-trivial.\n\
         - Call tools when needed; never invent tool results.\n";
    let always_on_path = identity_dir.join("_always-on.md");
    if !always_on_path.exists() {
        fs::write(&always_on_path, always_on)?;
    }

    let facts = "facts: []\n";
    let facts_path = identity_dir.join("facts.yaml");
    if !facts_path.exists() {
        fs::write(&facts_path, facts)?;
    }
    Ok(())
}

pub async fn register_in_process_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterInProcessAgentReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let id = req.id.trim().to_string();
    if id.is_empty() || id.contains('/') {
        return Err((
            StatusCode::BAD_REQUEST,
            "id must be non-empty and not contain '/'".to_string(),
        ));
    }
    if req.identity_dir.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "identity_dir is required".to_string(),
        ));
    }
    let identity_dir = PathBuf::from(req.identity_dir.trim());

    // Materialize the template so the user has something editable.
    if let Err(e) = materialize_agent_template(&id, &identity_dir).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to materialize identity template: {e}"),
        ));
    }

    let command = req
        .command
        .unwrap_or_else(|| "/home/andres_fernandez/.local/bin/agent".to_string());

    // EP-0011 S-005: command whitelist. Only known agent binaries are
    // allowed. Prevents command injection via `command: "/bin/sh"`.
    const ALLOWED_AGENT_COMMANDS: &[&str] = &["agent", "agent-template", "neurox-agent"];
    let cmd_basename = std::path::Path::new(&command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !ALLOWED_AGENT_COMMANDS.contains(&cmd_basename) {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "agent command '{}' not whitelisted (allowed: {:?})",
                cmd_basename, ALLOWED_AGENT_COMMANDS
            ),
        ));
    }

    let args = req
        .args
        .unwrap_or_else(|| vec!["--id".to_string(), id.clone()]);

    // Ensure the command uses --identity-dir pointing at the dir we just
    // (re)created, otherwise override an existing flag.
    let mut args = args;
    let mut has_id = false;
    let mut has_identity = false;
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == "--id" {
            args[i + 1] = id.clone();
            has_id = true;
        } else if args[i] == "--identity-dir" {
            args[i + 1] = identity_dir.to_string_lossy().into_owned();
            has_identity = true;
        }
        i += 1;
    }
    if !has_id {
        args.push("--id".to_string());
        args.push(id.clone());
    }
    if !has_identity {
        args.push("--identity-dir".to_string());
        args.push(identity_dir.to_string_lossy().into_owned());
    }

    let spec = SessionAgentSpec {
        command: command.clone(),
        args: args.clone(),
        env: std::collections::HashMap::new(),
        idle_timeout_secs: req.idle_timeout_secs.unwrap_or(1800),
        max_sessions: req.max_sessions,
        // EP-2026-08-15 (Fix 4): runtime registration does not yet
        // accept an allowlist. POST /v1/agents/in_process is the
        // ad-hoc create path; production agents are configured in
        // ~/.config/neurox/config.yaml where the allowlist lives.
        tools_allowlist: None,
        system_prompt: None,
        // No per-agent approval override at the runtime path: fall
        // back to the tool's `requires_approval` flag from llmd.
        requires_approval: None,
    };

    state.lifecycle.session_agents.register_spec(id.clone(), spec).await;

    info!(
        agent_id = %id,
        identity_dir = %identity_dir.display(),
        command = %command,
        "EP-2026-08-15: registered new in_process agent"
    );

    Ok(Json(json!({
        "registered": "in_process",
        "id": id,
        "identity_dir": identity_dir,
        "command": command,
        "args": args,
    })))
}

/// Deregister a runtime-registered agent. Active sessions already
/// bound to this agent_id continue running until they idle-evict or
/// get cancelled; new sessions with this agent_id get a
/// `no session_agent spec` error.
pub async fn deregister_in_process_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state.lifecycle.session_agents.deregister_spec(&id).await {
        Some(spec) => Ok(Json(json!({
            "deregistered": id,
            "command": spec.command,
        }))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("no in_process agent registered as '{id}'"),
        )),
    }
}

#[derive(Deserialize, Serialize)]
pub struct RegisterAgentReq {
    #[serde(rename = "type")]
    pub agent_type: String,
}

pub async fn register_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterAgentReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let agent_type = req.agent_type.as_str();

    match agent_type {
        "persistent" => {
            let body =
                serde_json::to_value(&req).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            let spec: PersistentAgentSpec = serde_json::from_value(body)
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            let id = spec.id.clone();
            state.lifecycle.registry.register_persistent(spec).await;
            Ok(Json(json!({"registered": "persistent", "id": id})))
        }
        "ephemeral_template" => {
            let body =
                serde_json::to_value(&req).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            let spec: EphemeralAgentSpec = serde_json::from_value(body)
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            let id = spec.id.clone();
            state.lifecycle.registry.register_ephemeral_template(spec).await;
            Ok(Json(json!({"registered": "ephemeral_template", "id": id})))
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("unknown agent type: {agent_type}"),
        )),
    }
}

/// Runtime view of an agent returned by `GET /v1/agents/:id` and
/// `PATCH /v1/agents/:id`. Fields are unified across the three
/// underlying registries (persistent, ephemeral_template, in_process)
/// and only the relevant ones are populated for each kind; the rest
/// serialize as `None` and are skipped thanks to
/// `#[serde(skip_serializing_if = "Option::is_none")]` on every
/// optional field. `system_prompt` is the headline use case — it
/// carries the inline prompt override from the `AgentSpec` so the
/// frontend can render/edit it from the Settings tab.
#[derive(Debug, Serialize)]
pub struct Agent {
    /// Agent kind: "persistent", "ephemeral_template", or "in_process".
    pub r#type: String,
    /// Stable id. Always populated for in-process agents and PATCH
    /// outcomes; mirrored for persistent/ephemeral_template so the
    /// frontend can pivot on it without re-parsing the spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Redacted spec for persistent / ephemeral_template agents and
    /// the session_agents PATCH echo. `None` for in-process GETs,
    /// which expose the relevant fields directly instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<serde_json::Value>,
    /// Status string for in-process agents (e.g. "ready").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Pool kind for in-process session_agent agents
    /// (e.g. "session-isolated").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Spawn command for in-process session_agent agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Spawn args for in-process session_agent agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// Idle timeout (s) for in-process session_agent agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout_secs: Option<u64>,
    /// Max concurrent sessions for in-process session_agent agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_sessions: Option<usize>,
    /// Tool allowlist for in-process session_agent agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_allowlist: Option<Vec<String>>,
    /// Inline system prompt override from the `AgentSpec`. `None`
    /// when the spec doesn't carry one (e.g. ephemeral_template and
    /// the legacy in-process default agent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// PATCH outcome: whether the update succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    /// PATCH outcome: whether a prior spec existed before the update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_exists: Option<bool>,
}

pub async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Agent>, (StatusCode, String)> {
    if let Some(spec) = state.lifecycle.registry.get_persistent(&id).await {
        let spec_json = serde_json::to_value(spec.redact_secrets())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(Agent {
            r#type: "persistent".to_string(),
            id: Some(id),
            spec: Some(spec_json),
            status: None,
            kind: None,
            command: None,
            args: None,
            idle_timeout_secs: None,
            max_sessions: None,
            tools_allowlist: None,
            system_prompt: spec.system_prompt.clone(),
            ok: None,
            previous_exists: None,
        }));
    }
    if let Some(spec) = state.lifecycle.registry.get_ephemeral_template(&id).await {
        let spec_json = serde_json::to_value(spec.redact_secrets())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(Agent {
            r#type: "ephemeral_template".to_string(),
            id: Some(id),
            spec: Some(spec_json),
            status: None,
            kind: None,
            command: None,
            args: None,
            idle_timeout_secs: None,
            max_sessions: None,
            tools_allowlist: None,
            // EphemeralAgentSpec doesn't carry a system_prompt — carries
            // its overrides via `kind` (subprocess env) instead.
            system_prompt: None,
            ok: None,
            previous_exists: None,
        }));
    }
    // EP-2026-08-15: also resolve session_agents specs (the new per-session
    // subprocess pool). The frontend AgentSelector dropdown talks to
    // /v1/agents/:id directly, so without this branch every click on
    // a session_agent item 404'd.
    if let Some(spec) = state.lifecycle.session_agents.spec(&id).await {
        return Ok(Json(Agent {
            r#type: "in_process".to_string(),
            id: Some(id),
            spec: None,
            status: Some("ready".to_string()),
            kind: Some("session-isolated".to_string()),
            command: Some(spec.command.clone()),
            args: Some(spec.args.clone()),
            idle_timeout_secs: Some(spec.idle_timeout_secs),
            max_sessions: spec.max_sessions,
            tools_allowlist: spec.tools_allowlist.clone(),
            system_prompt: spec.system_prompt.clone(),
            ok: None,
            previous_exists: None,
        }));
    }
    Err((StatusCode::NOT_FOUND, format!("agent not found: {id}")))
}

pub async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let removed_p = state.lifecycle.registry.remove_persistent(&id).await;
    let removed_e = state.lifecycle.registry.remove_ephemeral_template(&id).await;
    if removed_p || removed_e {
        Ok(Json(json!({"deleted": id})))
    } else {
        Err((StatusCode::NOT_FOUND, format!("agent not found: {id}")))
    }
}

pub async fn start_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let spec = state
        .lifecycle.registry
        .get_persistent(&id)
        .await
        .ok_or((StatusCode::NOT_FOUND, format!("agent not found: {id}")))?;

    state.lifecycle
        .supervisor
        .start_agent(spec)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({"started": id})))
}

pub async fn stop_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.lifecycle
        .supervisor
        .stop_agent(&id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(json!({"stopped": id})))
}

#[derive(Serialize)]
pub struct SessionCreated {
    session_id: Uuid,
    agent_id: String,
    /// How the daemon will execute this session's messages:
    /// - `session_process` → dedicated subprocess (per-session, real parallelism)
    /// - `persistent`      → long-lived shared subprocess (Supervisor)
    /// - `ephemeral`       → one-shot subprocess killed after each task
    /// - `in_process`      → legacy in-process default (singleton)
    #[serde(default)]
    executor: &'static str,
    /// PID of the per-session subprocess when `executor == "session_process"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    /// Stable client identifier (e.g. "web", "sidebar-<instance>").
    /// Sessions from different clients are partitioned in the
    /// session list so web and sidebar don't accidentally share
    /// session IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<SessionCreated>, (StatusCode, String)> {
    let agent_id = body
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "missing agent_id".to_string()))?
        .to_string();
    // SIDEBAR-FIX: the sidebar's useDefaultAgentId() can return null
    // if /v1/agents hasn't loaded yet, and ChatBubble falls back to
    // "" instead of waiting. An empty string here used to fall
    // through to is_known_agent("", ...) and return
    // "agent not found: " which the sidebar mis-rendered as
    // "Could not create neurox session (is the daemon running?)".
    // Reject empty up front so the error names the actual problem.
    if agent_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "agent_id is required (empty string sent — caller likely hadn't loaded /v1/agents yet)".to_string(),
        ));
    }
    let client_id = body
        .get("client_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if !state.is_known_agent(&agent_id).await {
        return Err((
            StatusCode::NOT_FOUND,
            format!("agent not found: {}", agent_id),
        ));
    }

    let session_id = Uuid::new_v4();

    // Auto-spawn the per-session subprocess when a spec exists. This is
    // the new path that gives real per-chat parallelism: each session
    // gets its own dedicated process.
    let (executor, pid) = if state.lifecycle.session_agents.is_session_agent(&agent_id).await {
        let agent = state.lifecycle
            .session_agents
            .start_for_session(session_id, &agent_id)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to spawn session agent: {e}"),
                )
            })?;
        ("session_process", agent.pid)
    } else {
        // Hint to the client about which executor will serve its messages.
        let persistent = state.lifecycle.registry.list_persistent().await;
        let ephemeral = state.lifecycle.registry.list_ephemeral_templates().await;
        let executor = if persistent.iter().any(|p| p.id == agent_id) {
            "persistent"
        } else if ephemeral.iter().any(|e| e.id == agent_id) {
            "ephemeral"
        } else {
            "in_process"
        };
        (executor, None)
    };

    state.emit(Event::SessionStarted {
        session_id,
        agent_id: agent_id.clone(),
    });

    // Persist session with the authenticated user as owner so the
    // session shows up in every device the user is logged into.
    // `start_session_for_user` also clears `ended_at` (via
    // INSERT OR REPLACE) so a re-opened session is truly active.
    let user_id = user.user_id.to_string();
    if let Err(e) = state
        .lifecycle
        .session
        .start_session_for_user(session_id, &agent_id, &user_id)
        .await
    {
        tracing::warn!(
            session_id = %session_id,
            user_id = %user_id,
            error = %e,
            "session persist failed (non-fatal)"
        );
    }
    // Telemetry tag — kept for diagnostic views (which device opened
    // the session). No longer used for list partitioning.
    if let Some(ref cid) = client_id {
        if let Err(e) = state.lifecycle.session.set_client_id(session_id, Some(cid)).await {
            tracing::warn!(session_id = %session_id, error = %e, "client_id set failed");
        }
    }

    // Auto-populate the session's LLM provider/model from the daemon's
    // active defaults so `/health` shows what the agent is actually
    // using without a separate `PUT /v1/sessions/:id/model` call.
    // MUST run after `start_session` because `set_model` UPDATEs an
    // existing row.
    // F4.4c: read the active provider/model from the engine.
    // EP-2026-08-19: no more HTTP hop — the engine is in-process.
    let v = state.engine.active_provider().await;
    let provider_id = v.get("provider_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if !provider_id.is_empty() {
        if let Err(e) = state.lifecycle.session.set_model(session_id, &provider_id, &model).await {
            tracing::warn!(session_id = %session_id, error = %e, "session model set failed");
        }
    } else {
        tracing::warn!(session_id = %session_id, "no active LLM provider/model — session model left empty");
    }

    Ok(Json(SessionCreated {
        session_id,
        agent_id,
        executor,
        pid,
        client_id,
    }))
}

#[derive(Deserialize)]
pub struct MessageReq {
    pub agent_id: String,
    pub text: String,
    /// LLM provider the user selected for THIS message. The session
    /// records it as the "last model used" and the subprocess uses it
    /// to (re)build the LlmClient if it differs from the current one.
    /// Optional for backward compat — falls back to the daemon's
    /// configured default provider (ChatBubble / old clients).
    #[serde(default)]
    pub provider_id: Option<String>,
    /// Model identifier within `provider_id`. Same semantics as above.
    /// Optional — falls back to the provider's configured default model.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional stable client id (e.g. "web", "sidebar-<instance>").
    /// Used to tag the dispatch with a `request_id` so concurrent
    /// streams on the same session don't cross-wire events.
    #[serde(default)]
    pub client_id: Option<String>,
    /// Client-supplied turn id (frontend uses `newRequestId()` →
    /// `"req_…"`). When present, the daemon echoes it back on every
    /// SSE chunk so the frontend's per-turn filter can drop
    /// cross-wired chunks. Falls back to a daemon-generated UUID when
    /// missing (back-compat with clients that don't send one).
    #[serde(default)]
    pub request_id: Option<String>,
}

#[tracing::instrument(skip_all, fields(session_id = %session_id, agent_id = tracing::field::Empty))]
pub async fn post_message(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
    Json(body): Json<MessageReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Owner check.
    let owner = state
        .lifecycle
        .session
        .get_session_user(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match owner {
        Some(uid) if uid == user.user_id.to_string() => {}
        _ => return Err((StatusCode::NOT_FOUND, "session not found".to_string())),
    }

    // Persist user message. Non-fatal if it fails. User messages never
    // carry thinking (EP-0026-rev-fix), so the column stays NULL.
    // On success, broadcast `MessageAppended` so other devices of the
    // same user see the prompt appear in their chat in realtime.
    match state
        .lifecycle
        .session
        .log_message(session_id, "user", &body.text, None)
        .await
    {
        Ok(msg_id) => {
            state.emit(Event::MessageAppended {
                session_id,
                message_id: msg_id,
                role: "user".to_string(),
                content: body.text.clone(),
                thinking: None,
                ts: chrono::Utc::now().to_rfc3339(),
            });
        }
        Err(e) => tracing::warn!(
            session_id = %session_id,
            error = %e,
            "user msg persist failed; skipping broadcast"
        ),
    }

    // Update session summary with first user message (preview for session list)
    let preview = if body.text.len() > 80 {
        let end = body
            .text
            .char_indices()
            .nth(80)
            .map(|(i, _)| i)
            .unwrap_or(body.text.len());
        format!("{}…", &body.text[..end])
    } else {
        body.text.clone()
    };
    let _ = state.lifecycle.session.update_summary(session_id, &preview).await;

    // Resolve the provider+model the user (or default) selected for this
    // message. Falls back to the daemon's configured default provider
    // (ChatBubble / old clients don't send them).
    let (provider_id, model) =
        resolve_default_model(&state, body.provider_id.as_deref(), body.model.as_deref()).await;

    // Persist the selected model on every message so the session always
    // reflects the last model the user actually used. UI restoration on
    // session load reads back `session.provider_id`/`session.model`.
    if let Err(e) = state
        .lifecycle
        .session
        .set_model(session_id, &provider_id, &model)
        .await
    {
        tracing::warn!(
            session_id = %session_id,
            error = %e,
            "failed to persist session model (non-fatal)"
        );
    }

    let tools_specs: Vec<serde_json::Value> = state.engine
        .tools
        .list_specs()
        .into_iter()
        .map(|spec| {
            json!({
                "type": "function",
                "function": {
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": spec.parameters,
                }
            })
        })
        .collect();

    let resolved = resolve_provider_runtime(&state, &provider_id, &model).await;
    let llm_param = resolved.as_ref().map(|r| {
        json!({
            "kind": r.kind.as_str(),
            "api_key": r.api_key,
            "base_url": r.base_url,
            "model": r.model,
        })
    });
    let params = json!({
        "text": body.text,
        "tools": tools_specs,
        "provider_id": provider_id,
        "model": model,
        "llm": llm_param,
    });
    let result = state
        .dispatch_to_agent(&body.agent_id, "process", session_id, params)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // EP-0026-rev-fix: persist assistant message (text + thinking if the
    // agent included it in the dispatch result). User messages never have
    // thinking so they pass `None` (see the call sites below).
    if let Some(text) = result
        .as_ref()
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())
    {
        let thinking = result
            .as_ref()
            .and_then(|r| r.get("thinking"))
            .and_then(|t| t.as_str())
            .map(str::to_string);
        match state
            .lifecycle
            .session
            .log_message(session_id, "assistant", text, thinking.as_deref())
            .await
        {
            Ok(msg_id) => {
                state.emit(Event::MessageAppended {
                    session_id,
                    message_id: msg_id,
                    role: "assistant".to_string(),
                    content: text.to_string(),
                    thinking: thinking.clone(),
                    ts: chrono::Utc::now().to_rfc3339(),
                });
            }
            Err(e) => tracing::warn!(
                session_id = %session_id,
                error = %e,
                "assistant msg persist failed; skipping broadcast"
            ),
        }
    }

    Ok(Json(json!({
        "session_id": session_id,
        "agent_id": body.agent_id,
        "result": result,
    })))
}

/// SSE streaming endpoint for chat messages.
///
/// Routes through the full default module (system prompt, tools, memory,
/// skills) — same as `post_message` but streams events as SSE.
/// EP-RAW-CHAT (2026-08-12): `POST /v1/chat/raw` — pure passthrough
/// to the upstream LLM provider's SSE stream, no transformation. No
/// auth, no DB writes, no agent loop — this is purely an inspection
/// tool so the user can see the wire format of MiniMax /
/// OpenAI-compat / Anthropic.
///
/// EP-2026-08-19: previously routed through llmd's `/v1/chat/raw`;
/// now the daemon makes the `reqwest` call directly against the
/// provider's base_url.
pub async fn post_chat_raw(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RawChatReq>,
) -> Response {
    use axum::body::Body;
    use futures::StreamExt;

    // Resolve (provider_id, model). Honour the body's values first;
    // otherwise ask the engine for the active provider.
    let active = state.engine.active_provider().await;
    let active_pid = active.get("provider_id").and_then(|v| v.as_str()).map(String::from);
    let active_model = active.get("model").and_then(|v| v.as_str()).map(String::from);

    let provider_id = body
        .provider_id
        .clone()
        .or(active_pid)
        .unwrap_or_else(|| "minimax".to_string());
    let model = body
        .model
        .clone()
        .or(active_model)
        .unwrap_or_else(|| "MiniMax-M3".to_string());

    // Look up the provider config for the base_url + api_key env name.
    let provider_cfg = match state.engine.list_providers().await {
        Ok(list) => list.into_iter().find(|p| p.id == provider_id),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("list_providers: {e}")})),
            )
                .into_response();
        }
    };
    let Some(provider_cfg) = provider_cfg else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("provider '{provider_id}' not found")})),
        )
            .into_response();
    };
    let base_url = provider_cfg.effective_base_url();
    let api_key = body
        .api_key
        .clone()
        .or_else(|| {
            provider_cfg
                .api_key_env
                .as_ref()
                .and_then(|env_name| std::env::var(env_name).ok())
        });

    // POST to <base_url>/chat/completions with stream:true.
    let mut url = base_url.clone();
    if !url.ends_with('/') {
        url.push('/');
    }
    url.push_str("chat/completions");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default();

    let req_body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": body.text}],
        "stream": true,
    });

    let mut req = client.post(&url).json(&req_body);
    if let Some(key) = &api_key {
        req = req.bearer_auth(key);
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("upstream: {e}")})),
            )
                .into_response();
        }
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let body_txt = resp.text().await.unwrap_or_default();
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("upstream {status}: {body_txt}")})),
        )
            .into_response();
    }

    let stream = resp.bytes_stream();
    let byte_stream = async_stream::stream! {
        let mut s = stream;
        while let Some(item) = s.next().await {
            match item {
                Ok(bytes) => yield Ok::<_, Infallible>(bytes),
                Err(_e) => break,
            }
        }
    };
    let body = Body::from_stream(byte_stream);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    (StatusCode::OK, headers, body).into_response()
}

#[derive(Debug, Deserialize)]
pub struct RawChatReq {
    pub text: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    /// Optional — kept so the SPA can pass the same UUID it uses for
    /// normal chat sessions. Raw mode does NOT persist; this is for
    /// convenience only.
    pub session_id: Option<Uuid>,
}

pub async fn post_message_stream(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
    Json(body): Json<MessageReq>,
) -> Response {
    // Owner check. 404 if the session isn't owned by the calling user.
    let owner = state.lifecycle.session.get_session_user(session_id).await;
    let owner_ok = matches!(owner, Ok(Some(ref uid)) if uid == &user.user_id.to_string());
    if !owner_ok {
        return session_not_found_response();
    }

    // EP-0012 P-005: bounded channel (64) prevents memory leak if the
    // SSE client is slow / disconnected. The forwarder blocks when
    // full; the dispatcher detects backpressure and exits.
    let (tx, rx) = mpsc::channel::<serde_json::Value>(64);

    // Persist user message (non-fatal if it fails). User messages never
    // carry thinking (EP-0026-rev-fix), so the column stays NULL.
    // On success, broadcast `MessageAppended` so other devices of the
    // same user see the prompt appear in their chat in realtime —
    // without this, the user's own messages only show on the device
    // that sent them (assistant responses DO propagate via the
    // stream events; user messages need their own broadcast).
    match state
        .lifecycle
        .session
        .log_message(session_id, "user", &body.text, None)
        .await
    {
        Ok(msg_id) => {
            state.emit(Event::MessageAppended {
                session_id,
                message_id: msg_id,
                role: "user".to_string(),
                content: body.text.clone(),
                thinking: None,
                ts: chrono::Utc::now().to_rfc3339(),
            });
        }
        Err(e) => tracing::warn!(
            session_id = %session_id,
            error = %e,
            "user msg persist failed; skipping broadcast"
        ),
    }

    // Update session summary with first user message
    let preview = if body.text.len() > 80 {
        let end = body
            .text
            .char_indices()
            .nth(80)
            .map(|(i, _)| i)
            .unwrap_or(body.text.len());
        format!("{}…", &body.text[..end])
    } else {
        body.text.clone()
    };
    let _ = state.lifecycle.session.update_summary(session_id, &preview).await;

    let session_for_task = session_id;
    // D3 fix: tag this dispatch with a unique request_id so concurrent
    // SSE streams on the same session can filter to their own events.
    // Prefer the client's `request_id` when provided (lets the
    // frontend's per-turn filter match); fall back to a daemon-generated
    // UUID when missing (back-compat with clients that don't send one).
    let request_id = body
        .request_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let client_id = body.client_id.clone();

    // Subscribe to the GLOBAL event bus (default::process_message emits here).
    let mut event_rx = state.events.event_tx.subscribe();
    let tx_for_forward = tx.clone();
    let request_id_for_forward = request_id.clone();
    let client_id_for_forward = client_id.clone();
    let broadcast_state = state.clone();
    tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(Event::Content { session_id, text }) if session_id == session_for_task => {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "content",
                            "text": text,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    // EP-2026-09-05 cross-device: also broadcast on the
                    // global event bus so OTHER devices (subscribed
                    // via /v1/events WS) see the chunk in realtime.
                    broadcast_state.emit(Event::Content {
                        session_id,
                        text,
                    });
                }
                Ok(Event::Thinking { session_id, text }) if session_id == session_for_task => {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "thinking",
                            "text": text,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    broadcast_state.emit(Event::Thinking {
                        session_id,
                        text,
                    });
                }
                Ok(Event::ToolCall {
                    session_id,
                    tool,
                    args,
                    iteration,
                }) if session_id == session_for_task => {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "tool_call",
                            "tool": tool,
                            "args": args,
                            "iteration": iteration,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    broadcast_state.emit(Event::ToolCall {
                        session_id,
                        tool,
                        args,
                        iteration,
                    });
                }
                Ok(Event::ToolResult {
                    session_id,
                    tool,
                    result,
                    iteration,
                }) if session_id == session_for_task => {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "tool_result",
                            "tool": tool,
                            "result": result,
                            "iteration": iteration,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    broadcast_state.emit(Event::ToolResult {
                        session_id,
                        tool,
                        result,
                        iteration,
                    });
                }
                Ok(Event::ApprovalRequest { request })
                    if request.session_id == session_for_task =>
                {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "approval_request",
                            "id": request.id,
                            "tool": request.tool,
                            "args": request.args,
                            "reason": request.reason,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Event::ApprovalResolved {
                    session_id,
                    approval_id,
                    tool,
                    decision,
                }) if session_id == session_for_task => {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "approval_resolved",
                            "id": approval_id,
                            "tool": tool,
                            "decision": decision,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Event::Done {
                    session_id,
                    text: _,
                }) if session_id == session_for_task => {
                    let _ = tx_for_forward.send(serde_json::json!({
                        "request_id": request_id_for_forward,
                        "client_id": client_id_for_forward,
                        "type": "done",
                    }));
                    let _ = tx_for_forward.send(serde_json::json!("[DONE]"));
                    break;
                }
                // D2 fix: Event::Error is just an event — keep the stream
                // open until the dispatcher's Event::Done closes the turn.
                // The previous behavior broke here, silently dropping the
                // Event::ToolResult that the dispatcher emits immediately
                // after the error.
                Ok(Event::Error {
                    session_id: Some(sid),
                    message,
                }) if sid == session_for_task => {
                    if tx_for_forward
                        .send(serde_json::json!({
                            "type": "error",
                            "message": message,
                            "request_id": request_id_for_forward,
                            "client_id": client_id_for_forward,
                        }))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
                _ => {}
            }
        }
    });

    // EP-2026-08-15 (live-switch streaming fix): the streaming endpoint
    // was going through llmd via HTTP, which has its OWN hardcoded
    // "Soy EVA" prompt and IGNORES the body's agent_id. This caused
    // every streaming response to say "Soy EVA" regardless of which
    // agent the user selected. The fix: route through the daemon's
    // own subprocess (agent binary via dispatch_to_agent), which loads
    // the correct identity dir + system prompt + facts + tools from
    // the per-agent filesystem. The dispatch also handles live agent
    // switching (kills the old subprocess, spawns the new one with
    // seed_history). Events flow through the global bus as before;
    // the forward_event_bus_to_sse loop above picks them up and
    // pushes them to the SSE response.
    let event_tx = state.events.event_tx.clone();
    let tx_done = tx.clone();
    let agent_id = body.agent_id.clone();
    let tools_specs: Vec<serde_json::Value> = state.engine
        .tools
        .list_specs()
        .into_iter()
        .map(|spec| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": spec.parameters,
                }
            })
        })
        .collect();
    // Resolve the provider+model the user (or default) selected for this
    // message. Falls back to the daemon's configured default provider
    // (ChatBubble / old clients don't send them).
    let (provider_id, model) =
        resolve_default_model(&state, body.provider_id.as_deref(), body.model.as_deref()).await;

    // Persist the selected model on every message so the session always
    // reflects the last model the user actually used. UI restoration on
    // session load reads back `session.provider_id`/`session.model`.
    if let Err(e) = state
        .lifecycle
        .session
        .set_model(session_id, &provider_id, &model)
        .await
    {
        tracing::warn!(
            session_id = %session_id,
            error = %e,
            "failed to persist session model (non-fatal)"
        );
    }

    let resolved = resolve_provider_runtime(&state, &provider_id, &model).await;
    let llm_param = resolved.as_ref().map(|r| {
        serde_json::json!({
            "kind": r.kind.as_str(),
            "api_key": r.api_key,
            "base_url": r.base_url,
            "model": r.model,
        })
    });
    let params = serde_json::json!({
        "text": body.text,
        "tools": tools_specs,
        "provider_id": provider_id,
        "model": model,
        "llm": llm_param,
    });
    let dispatch_state = state.clone();

    tokio::spawn(async move {
        let result = match dispatch_state
            .dispatch_to_agent(&agent_id, "process", session_for_task, params)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(session_id = %session_for_task, error = %e, "dispatch stream failed");
                let _ = event_tx.send(Event::Error {
                    session_id: Some(session_for_task),
                    message: e.to_string(),
                });
                let _ = tx_done.send(serde_json::json!("[DONE]"));
                return;
            }
        };

        // EP-2026-08-15 (live-switch streaming fix): dispatch_to_agent
        // already emitted Content/Thinking/ToolCall/ToolResult events
        // to the global bus during the stream. The forward loop at the
        // top of this handler picks them up and pushes them to the
        // SSE response. Here we only need to persist the final
        // assistant message (the post-stream equivalent of what
        // post_message does after dispatch).
        if let Some(text) = result
            .as_ref()
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
        {
            let thinking = result
                .as_ref()
                .and_then(|r| r.get("thinking"))
                .and_then(|t| t.as_str())
                .map(str::to_string);
            match state
                .lifecycle
                .session
                .log_message(session_for_task, "assistant", text, thinking.as_deref())
                .await
            {
                Ok(msg_id) => {
                    event_tx
                        .send(Event::MessageAppended {
                            session_id: session_for_task,
                            message_id: msg_id,
                            role: "assistant".to_string(),
                            content: text.to_string(),
                            thinking: thinking.clone(),
                            ts: chrono::Utc::now().to_rfc3339(),
                        })
                        .ok();
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_for_task,
                        error = %e,
                        "assistant msg persist failed; skipping broadcast"
                    );
                }
            }
        }
        let _ = event_tx.send(Event::Done {
            session_id: session_for_task,
            text: String::new(),
        });
        drop(tx_done); // ensure the SSE stream ends
    });

    sse_response(rx)
}

/// Extract the FIRST `<think>…</think>` block from an accumulated content
/// string. Returns the cleaned content (with the block removed) and the
/// inner text trimmed. If no complete block is found, the content is
/// returned unchanged and `None` is yielded.
/// Build an SSE response from a JSON-value stream.
fn sse_response(mut rx: mpsc::Receiver<serde_json::Value>) -> Response {
    let stream = async_stream::stream! {
        while let Some(value) = rx.recv().await {
            let payload = match value {
                serde_json::Value::String(s) if s == "[DONE]" => "[DONE]".to_string(),
                v => v.to_string(),
            };
            yield Ok::<_, Infallible>(format!("data: {payload}\n\n"));
        }
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Response::builder()
        .status(StatusCode::OK)
        .body(axum::body::Body::from_stream(stream))
        .unwrap()
}

pub async fn cancel_session(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Owner check: refuse to operate on sessions the caller doesn't
    // own. Same 404-not-leak convention as list/messages. Sessions
    // without a `user_id` (legacy or unowned) are NOT accessible —
    // strict ownership required. New sessions always have user_id via
    // `create_session` → `start_session_for_user`.
    let owner = state
        .lifecycle
        .session
        .get_session_user(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match owner {
        Some(uid) if uid == user.user_id.to_string() => {}
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "session not found".to_string(),
            ));
        }
    }

    // Trigger cooperative cancellation
    let was_active = state.lifecycle.tasks.cancel(session_id).await;
    state.lifecycle.approvals.cancel_session(session_id).await;
    // Kill the per-session subprocess if any. Idempotent on no-op.
    let agent_stopped = state.lifecycle
        .session_agents
        .stop(session_id)
        .await
        .map(|_| true)
        .unwrap_or(false);
    state.emit(Event::SessionEnded {
        session_id,
        summary: Some("cancelled".to_string()),
    });
    Ok(Json(json!({
        "cancelled": session_id,
        "was_active": was_active,
        "agent_stopped": agent_stopped,
    })))
}

/// GET /v1/sessions/:id — session detail (metadata + agent status + msg count).
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Owner check via the user_id index — single lookup, no list scan.
    let owner = state
        .lifecycle
        .session
        .get_session_user(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match owner {
        Some(uid) if uid == user.user_id.to_string() => {}
        _ => return Err((StatusCode::NOT_FOUND, "session not found".to_string())),
    }

    // We have the owner check, but for the actual metadata we still
    // need the session row. Cheap query (indexed by session_id PK).
    let sessions = state
        .lifecycle
        .session
        .list_sessions(1000)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let session = sessions
        .into_iter()
        .find(|s| s.session_id == session_id.to_string())
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".to_string()))?;
    let message_count = state.lifecycle
        .session
        .get_messages(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .len();
    let agent = state.lifecycle.session_agents.get(session_id).await;
    let agent_json = agent.as_ref().map(|a| {
        serde_json::json!({
            "agent_id": a.agent_id,
            "pid": a.pid,
            "command": a.spec.command,
            "args": a.spec.args,
            "uptime_secs": a.started_at.elapsed().as_secs(),
            "idle_secs": a.last_active.lock().elapsed().as_secs(),
            "max_idle_secs": a.spec.idle_timeout_secs,
            "max_sessions": a.spec.max_sessions,
        })
    });
    Ok(Json(serde_json::json!({
        "session_id": session.session_id,
        "agent_id": session.agent_id,
        "started_at": session.started_at,
        "ended_at": session.ended_at,
        "summary": session.summary,
        "message_count": message_count,
        "agent": agent_json,
    })))
}

/// DELETE /v1/sessions/:id — kill the session process and mark the session
/// as ended. Idempotent: a second call returns 404 on the session.
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Owner check: same strict-ownership convention as cancel_session.
    let owner = state
        .lifecycle
        .session
        .get_session_user(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match owner {
        Some(uid) if uid == user.user_id.to_string() => {}
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                "session not found".to_string(),
            ));
        }
    }

    let was_active = state.lifecycle.tasks.cancel(session_id).await;
    state.lifecycle.approvals.cancel_session(session_id).await;
    let agent_stopped = state.lifecycle.session_agents.stop(session_id).await.is_ok();
    // Permanently remove the session + its messages (real delete, not a
    // soft-close). This makes it disappear from GET /v1/sessions in both
    // the sidebar and web clients.
    let _ = state.lifecycle
        .session
        .delete_session_row(session_id)
        .await;
    state.emit(Event::SessionEnded {
        session_id,
        summary: Some("deleted".to_string()),
    });
    Ok(Json(json!({
        "deleted": session_id,
        "was_active": was_active,
        "agent_stopped": agent_stopped,
    })))
}

/// POST /v1/sessions/:id/reactivate — reopen a previously closed
/// session (sets `ended_at` back to NULL). Returns 404 if the
/// session doesn't exist OR isn't owned by the caller; returns
/// `{"reactivated": true}` on success. The subprocess isn't
/// spawned here — it spawns lazily on the next message via the
/// dispatch path (so a stale subprocess isn't left running if the
/// user reopens a session without sending anything).
pub async fn reactivate_session(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let reactivated = state
        .lifecycle
        .session
        .reactivate_session(session_id, &user.user_id.to_string())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !reactivated {
        return Err((
            StatusCode::NOT_FOUND,
            "session not found".to_string(),
        ));
    }
    state.emit(Event::SessionStarted {
        session_id,
        agent_id: String::new(), // unknown without an extra lookup; clients fetch full session via /v1/sessions/:id
    });
    Ok(Json(json!({ "reactivated": session_id })))
}

/// GET /v1/sessions/:id/agent — process details for the session's bound agent.
pub async fn get_session_agent(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let agent = state.lifecycle
        .session_agents
        .get(session_id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session agent not found".to_string()))?;
    // EP-2026-08-31: report dead subprocesses as 404 so the sidebar's
    // bash validator (and any other client) sees the session as gone
    // and recreates. Without this, the cached `pid` looks alive to
    // the validator and the next /send hangs on the dead pipe.
    if !crate::session_agents::SessionAgentPool::is_subprocess_alive(&agent) {
        return Err((
            StatusCode::NOT_FOUND,
            "session agent subprocess is dead".to_string(),
        ));
    }
    Ok(Json(serde_json::json!({
        "session_id": agent.session_id,
        "agent_id": agent.agent_id,
        "pid": agent.pid,
        "command": agent.spec.command,
        "args": agent.spec.args,
        "uptime_secs": agent.started_at.elapsed().as_secs(),
        "idle_secs": agent.last_active.lock().elapsed().as_secs(),
        "max_idle_secs": agent.spec.idle_timeout_secs,
        "max_sessions": agent.spec.max_sessions,
    })))
}

/// POST /v1/sessions/:id/agent/restart — kill + respawn the session's subprocess.
pub async fn restart_session_agent(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let agent_id = state.lifecycle
        .session_agents
        .get(session_id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session agent not found".to_string()))?
        .agent_id
        .clone();
    // Cancel any in-flight work first so the old process can exit cleanly.
    let _ = state.lifecycle.tasks.cancel(session_id).await;
    state.lifecycle.approvals.cancel_session(session_id).await;
    state.lifecycle
        .session_agents
        .stop(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let agent = state.lifecycle
        .session_agents
        .start_for_session(session_id, &agent_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("respawn failed: {e}")))?;
    Ok(Json(serde_json::json!({
        "session_id": session_id,
        "agent_id": agent.agent_id,
        "pid": agent.pid,
        "restarted": true,
    })))
}

/// GET /v1/sessions/agents — list all live session agents + configured specs.
pub async fn list_session_agents(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let running = state.lifecycle.session_agents.list().await;
    let specs_list = state.lifecycle.session_agents.list_specs().await;
    let specs: Vec<serde_json::Value> = specs_list
        .into_iter()
        .map(|(id, spec)| {
            // Redact env values for /v1 response — never leak secrets.
            let env_keys: Vec<&String> = spec.env.keys().collect();
            serde_json::json!({
                "agent_id": id,
                "command": spec.command,
                "args": spec.args,
                "env_keys": env_keys,
                "idle_timeout_secs": spec.idle_timeout_secs,
                "max_sessions": spec.max_sessions,
            })
        })
        .collect();
    Json(serde_json::json!({
        "running": running,
        "specs": specs,
    }))
}

pub async fn list_approvals(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let pending = state.lifecycle.approvals.list().await;
    Json(json!({"pending": pending}))
}

/// List all registered tools with their specs (name, description, parameters
/// to render the Tools Explorer without hardcoding tool names. EP-0001.
/// EP-frontend-config: also returns an `enabled` flag per tool so the
/// frontend can render toggleable rows. Tools disabled via
/// `POST /v1/tools/:name/disable` stay listed (so the operator can
/// re-enable them) but report `enabled: false`.
/// PR-11: invoke a registered tool by name. The body is a JSON object whose
/// shape matches the tool's `parameters` schema. Triggers the existing
/// `Tool::execute(path)` implementation — for plugin-registered tools this
/// forwards to the plugin's HTTP surface via `PluginProxyTool`.
///
/// Approval gating: the chat-side flow (`handle_session_tool_call`) blocks
/// on `requires_approval=true` tools and prompts the user. For this HTTP
/// endpoint we deliberately skip approval — admins calling tools directly
/// are already trusted, and the chat path remains the gate for the LLM.
///
/// Status codes:
///   200 — `{ok: true,  result: <string>}` on success

#[derive(Deserialize)]
pub struct ApprovalResponseReq {
    pub decision: crate::approval::ApprovalDecision,
}

pub async fn respond_approval(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApprovalResponseReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Look up the pending approval BEFORE responding so we can capture its
    // session_id and tool name for the ApprovalResolved event. We then
    // remove it via the manager's `respond` call. If it was already removed
    // (race with a timeout/cancel), we return 404.
    let pending = state.lifecycle.approvals.get(id).await;
    let Some(req) = pending else {
        return Err((StatusCode::NOT_FOUND, format!("approval not found: {id}")));
    };
    let resolved = state.lifecycle.approvals.respond(id, body.decision).await;
    if !resolved {
        return Err((StatusCode::NOT_FOUND, format!("approval not found: {id}")));
    }
    state.emit(Event::ApprovalResolved {
        approval_id: id,
        session_id: req.session_id,
        tool: req.tool,
        decision: match body.decision {
            crate::approval::ApprovalDecision::Approve => "approve".to_string(),
            crate::approval::ApprovalDecision::Deny => "deny".to_string(),
        },
    });
    Ok(Json(
        json!({"resolved": id, "decision": match body.decision {
            crate::approval::ApprovalDecision::Approve => "approve",
            crate::approval::ApprovalDecision::Deny => "deny",
        }}),
    ))
}

/// PUT /v1/sessions/:id/rename — rename a session.
pub async fn rename_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "missing 'name' field".to_string()))?;

    state.lifecycle
        .session
        .rename_session(session_id, name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "renamed": session_id, "name": name })))
}

#[derive(Deserialize)]
pub struct SetSessionModelReq {
    pub provider_id: String,
    pub model: String,
}

/// PUT /v1/sessions/:id/model — set per-session LLM provider+model (EP-0017-02 R3).
///
/// Validates that the provider exists and is configured (has its API key set,
/// or does not require one). Persists via `SessionStore::set_model`, so it
/// applies to both new and existing sessions.
pub async fn set_session_model(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SetSessionModelReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Owner check. Same 404 convention as cancel/delete.
    let owner = state
        .lifecycle
        .session
        .get_session_user(session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;
    match owner {
        Some(uid) if uid == user.user_id.to_string() => {}
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "session not found"})),
            ));
        }
    }
    // F5.1 runtime fix: the provider may live in llmd's SQLite
    // (created via the CRUD endpoints) rather than the daemon's
    // YAML config. Best-effort check: ask llmd first, fall back
    // to the YAML config. The actual existence is validated when
    // the chat fires. Skip the "config check" entirely — llmd
    // validates the API key when the chat actually runs.
    let llmd_provider = state
        .engine
        .get_provider(&body.provider_id)
        .await
        .ok()
        .flatten();
    let yaml_provider = state.config.llm.get_provider(&body.provider_id).cloned();
    if llmd_provider.is_none() && yaml_provider.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("provider '{}' not found", body.provider_id)})),
        ));
    }

    // `set_model` errors with "session not found" when the row doesn't
    // exist yet. This happens on tabs whose sessionId came from
    // localStorage (useChatTabs) but the daemon's DB doesn't have that
    // session anymore (DB reset, different client partition, etc.).
    // Instead of returning 404 and forcing the user to send a message
    // first, auto-create the session row with the default in-process
    // agent and retry. The full session init (subprocess spawn, etc.)
    // still happens lazily on first message via `create_session`.
    let set_result = state
        .lifecycle
        .session
        .set_model(session_id, &body.provider_id, &body.model)
        .await;
    if let Err(e) = set_result {
        if !e.to_string().contains("session not found") {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            ));
        }
        if let Err(e2) = state
            .lifecycle
            .session
            .start_session_for_user(session_id, "default", &user.user_id.to_string())
            .await
        {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("auto-create session: {e2}")})),
            ));
        }
        state
            .lifecycle
            .session
            .set_model(session_id, &body.provider_id, &body.model)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
            })?;
    }

    Ok(Json(json!({
        "session_id": session_id,
        "provider_id": body.provider_id,
        "model": body.model,
    })))
}

// ─── EP-0016: per-session UI config endpoints ───────────────────────────

#[derive(Deserialize)]
pub struct SetSessionUiModeReq {
    /// `"plan"` | `"build"`. `null` clears the session override
    /// (the daemon default applies).
    pub mode: Option<String>,
}

/// PUT /v1/sessions/:id/mode — set the UI mode for a session (EP-0016).
///
/// Affects system prompt suffix and tool availability at the agent
/// level. `None` clears the per-session override.
pub async fn set_session_mode(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SetSessionUiModeReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mode_ref = body.mode.as_deref();
    state
        .lifecycle
        .session
        .set_ui_mode(session_id, mode_ref)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("session not found") {
                (StatusCode::NOT_FOUND, msg)
            } else {
                (StatusCode::BAD_REQUEST, msg)
            }
        })?;

    Ok(Json(json!({
        "session_id": session_id,
        "mode": body.mode,
    })))
}

#[derive(Deserialize)]
pub struct SetSessionToolModeReq {
    /// `"functions"` | `"search"` | `"none"`. `null` clears the override.
    pub mode: Option<String>,
}

/// PUT /v1/sessions/:id/tool-mode — set the tool selection mode (EP-0016).
///
/// Drives the `tool_choice` field in the LLM request:
/// - `functions` → `"auto"` (model picks from registered tools)
/// - `search`    → only web search tool exposed
/// - `none`      → no tools
pub async fn set_session_tool_mode(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SetSessionToolModeReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mode_ref = body.mode.as_deref();
    state
        .lifecycle
        .session
        .set_tool_mode(session_id, mode_ref)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("session not found") {
                (StatusCode::NOT_FOUND, msg)
            } else {
                (StatusCode::BAD_REQUEST, msg)
            }
        })?;

    let tool_choice = match body.mode.as_deref() {
        Some("search") => "web_search",
        Some("none") => "none",
        _ => "auto",
    };

    Ok(Json(json!({
        "session_id": session_id,
        "mode": body.mode,
        "tool_choice": tool_choice,
    })))
}

#[derive(Deserialize)]
pub struct SetSessionTemperatureReq {
    /// 0.0–2.0. `null` clears the override.
    pub value: Option<f64>,
}

/// PUT /v1/sessions/:id/temperature — set the sampling temperature (EP-0016).
pub async fn set_session_temperature(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<SetSessionTemperatureReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .lifecycle
        .session
        .set_temperature(session_id, body.value)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("session not found") {
                (StatusCode::NOT_FOUND, msg)
            } else {
                (StatusCode::BAD_REQUEST, msg)
            }
        })?;

    Ok(Json(json!({
        "session_id": session_id,
        "value": body.value,
    })))
}

/// Information about a connected client (HTTP or WebSocket). Shaped
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientInfo {
    pub id: String,
    pub kind: String,        // "http" | "ws_events" | "ws_commands"
    pub detected_as: String, // "web" | "cli" | "tui" | "gtk" | "unknown"
    pub path: String,
    pub method: String,
    pub source_addr: String,
    pub user_agent: Option<String>,
    pub connected_at: String,
    pub duration_seconds: u64,
    pub requests_count: u64,
}

impl ClientInfo {
    /// Heuristic: detect the client kind from the User-Agent and path.
    pub fn detect(user_agent: Option<&str>, path: &str) -> String {
        let ua = user_agent.unwrap_or("").to_lowercase();
        let path_lc = path.to_lowercase();
        if ua.contains("mozilla")
            || ua.contains("chrome")
            || ua.contains("safari")
            || ua.contains("firefox")
            || ua.contains("webkit")
        {
            "web".into()
        } else if ua.contains("tui") || path_lc.contains("/tui") {
            "tui".into()
        } else if ua.contains("gtk") || path_lc.contains("/gtk") {
            "gtk".into()
        } else if ua.contains("curl") || ua.contains("wget") || ua.contains("httpie") {
            "cli".into()
        } else {
            "unknown".into()
        }
    }
}

// ─── EP-0009-05: LLM providers endpoints ────────────────────────────────────

/// GET /v1/llm/providers — list configured providers with active status.
/// EP-0017-02 R2: `active` is derived and equals `configured` (all configured
/// providers are available for per-session selection; there is no stored
/// activation field). The top-level `default_provider`/`default_model` are the
/// daemon fallback used by sessions without an explicit model.
/// EP-0018-03 R3: each provider with `local_command` adds a `service_state`
/// field sourced from the local service orchestrator. Remote providers get
/// `service_state: null`.
pub async fn list_llm_providers(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // F5.1 runtime fix: previously read from `state.config.llm.providers`
    // (the YAML config loaded at startup). After the F5.1 migration,
    // LLMs live in llmd's SQLite store. POST/DELETE proxy to llmd; this
    // GET must also proxy so the list reflects runtime CRUD.
    let providers_list = state.engine.list_providers().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "engine unreachable, returning empty provider list");
        Vec::new()
    });
    let mut providers: Vec<serde_json::Value> = Vec::with_capacity(providers_list.len());
    for p in providers_list {
        let id = p.id.clone();
        let kind = p.kind.as_str();
        let model = p.model.clone();
        let base_url = p.base_url.clone();
        let api_key_env = p.api_key_env.clone();
        let configured = catalog_is_configured_field(api_key_env.as_deref());
        // service_state: only for local providers. Stays null for now
        // (the orchestrator lookup requires the LlmProviderConfig struct
        // which we no longer have here).
        let service_state = serde_json::Value::Null;
        providers.push(json!({
            "id": id,
            "kind": kind,
            "model": model,
            "base_url": base_url,
            "api_key_env": api_key_env,
            "configured": configured,
            "active": configured,
            "service_state": service_state,
        }));
    }

    let active = state.engine.active_provider().await;
    let default_provider = active
        .get("provider_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let default_model = active
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Json(json!({
        "providers": providers,
        "default_provider": default_provider,
        "default_model": default_model,
    }))
}

/// Lightweight `configured` check derived from the optional
/// `api_key_env` field. The original `catalog_is_configured` takes
/// `&LlmProviderConfig`; since we now only have JSON, this is the
/// alphabetical helper.
fn catalog_is_configured_field(api_key_env: Option<&str>) -> bool {
    // A provider is "configured" when its api_key_env names an
    // environment variable that is actually set to a non-empty value —
    // not merely when the name string is present. Otherwise every
    // provider would report configured=true before the user ever
    // entered a key.
    match api_key_env {
        Some(name) if !name.is_empty() => std::env::var(name)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
        _ => false,
    }
}

/// Snake_case slug for `LlmProviderKind` (matches the serde rename used in
/// config). Centralised so the GET response stays consistent with the
/// config format (`minimax` / `openai_compat` / `anthropic`).
fn kind_slug(kind: tools_engine::LlmProviderKind) -> &'static str {
    match kind {
        tools_engine::LlmProviderKind::Minimax => "minimax",
        tools_engine::LlmProviderKind::OpenaiCompat => "openai_compat",
        tools_engine::LlmProviderKind::Anthropic => "anthropic",
    }
}

/// Snake-case slug for `ServiceState` (EP-0018-03 R3). Matches the serde
/// rename used in the orchestrator.
fn service_state_slug(state: crate::llm_admin::ServiceState) -> &'static str {
    use crate::llm_admin::ServiceState as S;
    match state {
        S::Stopped => "stopped",
        S::Starting => "starting",
        S::Running => "running",
        S::Ready => "ready",
        S::Failed => "failed",
    }
}

// ─── EP-0018-03: local service start/stop endpoints ──────────────────────

/// Resolve the provider config for a start/stop request. Returns the
/// configured provider, or an HTTP error mapping (404 if missing, 400 if
/// it has no `local_command`).
fn resolve_local_provider(
    state: &AppState,
    provider_id: &str,
) -> Result<crate::config::LlmProviderConfig, (axum::http::StatusCode, String)> {
    let cfg = state
        .config
        .llm
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .cloned()
        .ok_or_else(|| {
            (
                axum::http::StatusCode::NOT_FOUND,
                format!("provider '{provider_id}' not found"),
            )
        })?;
    if cfg.local_command.is_none() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            format!("provider '{provider_id}' has no local service"),
        ));
    }
    Ok(cfg)
}

/// POST /v1/llm/providers/:id/start — start the local service for a provider.
pub async fn start_local_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let cfg = resolve_local_provider(&state, &id)?;
    let orchestrator = state.auth.orchestrator.as_ref().ok_or_else(|| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "orchestrator not available".to_string(),
        )
    })?;
    // Make sure the orchestrator knows about this provider (it may have
    // missed `startup_discover` if the config was loaded after start).
    orchestrator.register(&cfg).await;
    orchestrator.start(&cfg.id).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to start local service: {e}"),
        )
    })?;

    // Provider state changed → rebuild catalog on next GET /v1/llm/models.
    crate::llm_admin::catalog::invalidate_llm_catalog(&state.workspace.llm_catalog_cache).await;
    let status = orchestrator.status(&cfg.id).await.unwrap_or_else(|| {
        // Just-registered; the lifecycle will populate it shortly.
        crate::llm_admin::LocalServiceStatus {
            provider_id: cfg.id.clone(),
            desired: crate::llm_admin::DesiredState::Running,
            state: crate::llm_admin::ServiceState::Starting,
            pid: None,
            attempts: 0,
            next_retry_at: None,
            last_error: None,
            last_health_check: None,
        }
    });
    Ok(Json(json!({
        "id": cfg.id,
        "service_state": service_state_slug(status.state),
    })))
}

/// POST /v1/llm/providers/:id/stop — stop the local service for a provider.
pub async fn stop_local_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let cfg = resolve_local_provider(&state, &id)?;
    let orchestrator = state.auth.orchestrator.as_ref().ok_or_else(|| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "orchestrator not available".to_string(),
        )
    })?;
    orchestrator.register(&cfg).await;
    orchestrator.stop(&cfg.id).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to stop local service: {e}"),
        )
    })?;

    // Provider state changed → rebuild catalog on next GET /v1/llm/models.
    crate::llm_admin::catalog::invalidate_llm_catalog(&state.workspace.llm_catalog_cache).await;

    Ok(Json(json!({
        "id": cfg.id,
        "service_state": "stopped",
    })))
}

#[derive(Deserialize)]
pub struct SetActiveProviderReq {
    pub provider_id: String,
    /// Explicit default model id (e.g. `"mistral-medium-latest"`).
    /// When present, persisted as the daemon-wide default and
    /// auto-populated on newly-created sessions.
    #[serde(default)]
    pub model: Option<String>,
}

/// PUT /v1/llm/providers/active — DEPRECATED (EP-0017-02 R4).
///
/// The endpoint is kept for backward compatibility with older TUI/admin
/// clients, but its semantics changed: it now updates the daemon-wide
/// *default* (`default_provider_id`/`default_model` in `DefaultAgentState`), which
/// is the fallback used by sessions without an explicit model. All configured
/// providers stay active for per-session selection.
///
/// Use `PUT /v1/sessions/:id/model` for per-session selection instead.
pub async fn set_active_llm_provider(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetActiveProviderReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Sets the daemon-wide default provider/model. The frontend
    // mostly uses PUT /v1/sessions/:id/model for per-session
    // selection now, but this endpoint is still the source of truth
    // for "the active default" reported by /v1/llm/providers and
    // used by newly-created sessions.
    //
    // Look up the provider in the engine store (where runtime CRUD
    // providers live) — `state.config.llm` only has the YAML-loaded
    // initial set, so checking there misses anything added later.
    let provider = match state.engine.list_providers().await {
        Ok(list) => list.into_iter().find(|p| p.id == body.provider_id),
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("list_providers: {e}")})),
            ));
        }
    };
    let Some(p) = provider else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "provider not found"})),
        ));
    };

    // When the client doesn't send a model, fall back to the
    // provider's `effective_model()` (its YAML/CRUD-configured
    // default). This keeps `set_active_provider` backwards-compat
    // with clients that only send `{provider_id}`.
    let model = body
        .model
        .clone()
        .unwrap_or_else(|| p.effective_model());

    state
        .engine
        .set_active_provider(p.id.clone(), model.clone())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    Ok(Json(json!({
        "active": body.provider_id,
        "model": model,
    })))
}

/// POST /v1/llm/providers/:id/test — test connectivity with a provider.
pub async fn test_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let provider = state.config.llm.get_provider(&id);
    let Some(p) = provider else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "provider not found"})),
        ));
    };

    let api_key = match &p.api_key_env {
        Some(env_name) => std::env::var(env_name).ok(),
        None => None,
    };

    let base_url = p.effective_base_url();
    let model = p.effective_model();

    // F4.4c: probe via llmd HTTP. Use the kind's snake_case wire form;
    // `format!("{:?}", p.kind)` would emit the PascalCase variant name
    // and llmd would reject it with `unknown provider kind: Minimax`.
    match state
        .engine
        .test_provider(p.id.clone(), p.kind.as_str(), api_key, base_url, model)
        .await
    {
        Ok(v) => Ok(Json(v)),
        Err(error) => Ok(Json(json!({
            "ok": false,
            "error": error,
        }))),
    }
}

// ─── EP-0010: LLM Provider CRUD endpoints ───────────────────────────────────

/// Request body for creating a new LLM provider.
#[derive(Deserialize)]
pub struct CreateProviderReq {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub extra: std::collections::HashMap<String, String>,
    // EP-0018: local service orchestration (opt-in).
    #[serde(default)]
    pub local_command: Option<String>,
    #[serde(default)]
    pub local_args: Vec<String>,
    #[serde(default)]
    pub local_model_path: Option<String>,
    #[serde(default)]
    pub local_port: Option<u16>,
}

/// Request body for updating an existing LLM provider.
#[derive(Deserialize)]
pub struct UpdateProviderReq {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub extra: Option<std::collections::HashMap<String, String>>,
    // EP-0018: local service orchestration (opt-in; None keeps current value).
    #[serde(default)]
    pub local_command: Option<Option<String>>,
    #[serde(default)]
    pub local_args: Option<Vec<String>>,
    #[serde(default)]
    pub local_model_path: Option<Option<String>>,
    #[serde(default)]
    pub local_port: Option<Option<u16>>,
}

/// Validate that a string is a syntactically valid env var name
/// (POSIX: `[A-Za-z_][A-Za-z0-9_]*`, plus uppercase convention). Used to
/// reject API key VALUES (e.g. `sk-or-...`) that were sent in the
/// `api_key_env` field by mistake — the backend looks up the env var by
/// name at discovery time, so a value would silently break discovery.
fn is_valid_env_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// POST /v1/llm/providers — create a new provider (201/409/400).
pub async fn create_llm_provider(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProviderReq>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let _store = &state.engine.providers;

    // Validate id
    if body.id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "id is required"})),
        ));
    }

    // Validate kind
    let kind = parse_provider_kind(&body.kind).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid kind '{}'. Must be one of: minimax, openai_compat, anthropic", body.kind)})),
        )
    })?;

    // Validate api_key_env if provided.
    //   - Reject if it doesn't look like an env var name (`sk-...` is the
    //     API key VALUE, not the env var NAME). The backend looks up the
    //     env var by name to read the key at discovery time, so a value
    //     here would silently break discovery forever.
    //   - Warn if the env var is not set in the current process.
    if let Some(ref env_name) = body.api_key_env {
        if !env_name.is_empty() {
            if !is_valid_env_var_name(env_name) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": format!(
                            "api_key_env '{}' is not a valid env var name \
                             (must match ^[A-Z][A-Z0-9_]*$). Did you pass the API key \
                             value instead of the env var name?",
                            env_name
                        )
                    })),
                ));
            }
            if std::env::var(env_name).is_err() {
                tracing::warn!(
                    provider_id = %body.id,
                    env_var = %env_name,
                    "creating provider with unconfigured api_key_env"
                );
            }
        }
    }

    let provider = tools_engine::LlmProviderConfig {
        id: body.id.clone(),
        kind: tools_engine::LlmProviderKind::from_str(
            crate::config::LlmProviderKind::as_str(&kind),
        )
        .unwrap_or(tools_engine::LlmProviderKind::OpenaiCompat),
        base_url: body.base_url,
        model: body.model,
        api_key_env: body.api_key_env,
        extra: body.extra,
        local_command: body.local_command,
        local_args: body.local_args,
        local_model_path: body.local_model_path,
        local_port: body.local_port,
    };

    state.engine.create_provider(&provider).await.map_err(|e| {
        if e.contains("already exists") {
            (StatusCode::CONFLICT, Json(json!({"error": e})))
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e})))
        }
    })?;

    // Provider set changed → rebuild catalog on next GET /v1/llm/models.
    crate::llm_admin::catalog::invalidate_llm_catalog(&state.workspace.llm_catalog_cache).await;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": provider.id,
            "kind": kind_slug(provider.kind),
            "base_url": provider.effective_base_url(),
            "model": provider.effective_model(),
            "api_key_env": provider.api_key_env,
        })),
    ))
}

/// PUT /v1/llm/providers/:id — update an existing provider (200/400/404).
pub async fn update_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProviderReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let store = &state.engine.providers;

    // Load existing provider to merge fields
    let existing = store
        .get(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    let Some(existing) = existing else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("provider '{}' not found", id)})),
        ));
    };

    let updated = tools_engine::LlmProviderConfig {
        id: existing.id.clone(),
        kind: existing.kind, // kind is immutable
        base_url: body.base_url.unwrap_or(existing.base_url),
        model: body.model.unwrap_or(existing.model),
        api_key_env: match body.api_key_env {
            Some(v) => {
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            }
            None => existing.api_key_env,
        },
        extra: body.extra.unwrap_or(existing.extra),
        local_command: body.local_command.unwrap_or(existing.local_command),
        local_args: body.local_args.unwrap_or(existing.local_args),
        local_model_path: body.local_model_path.unwrap_or(existing.local_model_path),
        local_port: body.local_port.unwrap_or(existing.local_port),
    };

    state
        .engine
        .update_provider(&id, &updated)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    // Provider config may have changed (active flag, api_key_env) → rebuild catalog.
    crate::llm_admin::catalog::invalidate_llm_catalog(&state.workspace.llm_catalog_cache).await;

    Ok(Json(json!({
        "id": updated.id,
        "kind": kind_slug(updated.kind),
        "base_url": updated.effective_base_url(),
        "model": updated.effective_model(),
        "api_key_env": updated.api_key_env,
    })))
}

/// DELETE /v1/llm/providers/:id — remove a provider (204/404/409).
pub async fn delete_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    // F5.1: cannot delete the active provider.
    let active_id = state
        .engine
        .active_provider()
        .await
        .get("provider_id")
        .and_then(|x| x.as_str().map(String::from));
    if active_id.as_deref() == Some(id.as_str()) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "cannot delete the active provider — swap to another first"})),
        ));
    }

    state.engine.delete_provider(&id).await.map_err(|e| {
        if e.contains("last provider") || e.contains("409") {
            (
                StatusCode::CONFLICT,
                Json(json!({"error": "cannot delete the last provider — at least one must remain"})),
            )
        } else if e.contains("404") {
            (StatusCode::NOT_FOUND, Json(json!({"error": "provider not found"})))
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e})))
        }
    })?;

    // Provider removed → rebuild catalog on next GET /v1/llm/models.
    crate::llm_admin::catalog::invalidate_llm_catalog(&state.workspace.llm_catalog_cache).await;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /v1/llm/providers/:id/models — discover available models for a provider.
pub async fn list_provider_models(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // EP-2026-08-19: previously proxied to llmd via HTTP. Now the
    // engine exposes `discover_models(provider_id)` directly.
    let models = state.engine.discover_models(&id).await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("discover_models: {e}")})),
        )
    })?;
    Ok(Json(json!({
        "provider_id": id,
        "models": models,
    })))
}

/// GET /v1/llm/models — accumulated model catalog across all configured
/// providers (EP-0017-02 R1).
///
/// The catalog is the union of every provider's discovered models, filtered
/// to `configured == true`. Individual discovery failures are skipped (they
/// never fail the catalog). Results are cached in memory for 300s.
pub async fn get_llm_models(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    use crate::llm_admin::catalog::{build_catalog, CATALOG_TTL_SECS};

    let now = Utc::now();
    // Cache hit: valid TTL → respond without re-querying providers.
    {
        let cache = state.workspace.llm_catalog_cache.lock().await;
        if let Some(cat) = cache.as_ref() {
            let age = (now - cat.fetched_at).num_seconds();
            if age < CATALOG_TTL_SECS {
                return Json(json!({
                    "models": cat.models,
                    "cached": true,
                    "fetched_at": cat.fetched_at.to_rfc3339(),
                }));
            }
        }
    }

    // Source: SQLite store when present (EP-0010 source of truth), else the
    // static config providers (e.g. tests / no store).
    let providers: Vec<tools_engine::LlmProviderConfig> = state.engine.list_providers().await.unwrap_or_default();

    let catalog = build_catalog(&providers).await;
    let fetched_at = catalog.fetched_at;
    let models = catalog.models.clone();
    *state.workspace.llm_catalog_cache.lock().await = Some(catalog);

    Json(json!({
        "models": models,
        "cached": false,
        "fetched_at": fetched_at.to_rfc3339(),
    }))
}

/// POST /v1/llm/providers/:id/ping — test provider connectivity (store-backed).
/// Like `/test` but reads the provider from SQLite, not static config.
/// Accepts optional `{ model }` in body to override the test model.
pub async fn ping_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // F5.1: look up the provider via llmd rather than the daemon's
    // SQLite store. The daemon still builds the test request from
    // the resolved provider (the actual ping is proxied to llmd
    // via llmd_client.test_provider below).
    let provider = state
        .engine
        .get_provider(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    let Some(p) = provider else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("provider '{}' not found", id)})),
        ));
    };

    let api_key = p
        .api_key_env
        .as_deref()
        .and_then(|s| if s.is_empty() { None } else { std::env::var(s).ok() });

    let base_url = if !p.base_url.is_empty() {
        p.base_url.clone()
    } else {
        p.effective_base_url()
    };
    let model = body
        .and_then(|b| b.get("model").and_then(|m| m.as_str().map(String::from)))
        .unwrap_or_else(|| p.effective_model());

    let provider_id = p.id.clone();
    let kind = p.kind.as_str();
    match state
        .engine
        .test_provider(provider_id.clone(), kind, api_key, base_url, model)
        .await
    {
        Ok(v) => {
            // Inject provider_id into the response for backward compat.
            let mut v = v;
            if let Some(obj) = v.as_object_mut() {
                obj.insert("provider_id".to_string(), serde_json::Value::String(id.clone()));
            }
            Ok(Json(v))
        }
        Err(error) => Ok(Json(json!({
            "ok": false,
            "provider_id": id,
            "error": error,
        }))),
    }
}

/// Parse provider kind from string. Accepts both `openai_compat` and `openai`.
fn parse_provider_kind(s: &str) -> Option<crate::config::LlmProviderKind> {
    match s {
        "minimax" => Some(crate::config::LlmProviderKind::Minimax),
        "openai_compat" | "openai" => Some(crate::config::LlmProviderKind::OpenaiCompat),
        "anthropic" => Some(crate::config::LlmProviderKind::Anthropic),
        _ => None,
    }
}

// ─── EP-0009: Dynamic plugin registration endpoints ─────────────────────────

/// POST /v1/mcps — register a plugin dynamically.
pub async fn register_plugin(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::auth::UserContext>,
    Json(body): Json<crate::plugins::PluginRegisterRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // EP-0011 S-009: plugin registration is Admin-only. Operators can
    // list plugins but not register new ones (would allow arbitrary
    // tool injection).
    if ctx.role != crate::auth::Role::Admin {
        return Err((
            StatusCode::FORBIDDEN,
            "admin role required for plugin registration".to_string(),
        ));
    }

    if body.name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "mcp name is required".into()));
    }
    if body.base_url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "base_url is required".into()));
    }
    let plugin_state = state.lifecycle.plugin_registry.register(body);
    state.emit(Event::McpRegistered {
        name: plugin_state.name.clone(),
        tools: plugin_state.tools.clone(),
        skills: plugin_state.skills.clone(),
    });
    Ok(Json(json!({
        "registered": plugin_state.name,
        "tools": plugin_state.tools,
        "skills": plugin_state.skills,
        "status": plugin_state.status,
    })))
}

/// POST /v1/mcps/:name/reconnect — attempt to reconnect to a plugin.
pub async fn reconnect_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.lifecycle
        .plugin_registry
        .reconnect(&name)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(json!({"reconnected": name})))
}

/// GET /v1/mcps — list all registered plugins with their state.
pub async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let plugins = state.engine.tools.list_specs();
    Json(json!({"plugins": plugins}))
}

/// GET /v1/mcps/catalog — list every plugin available in the
/// configured registry (`plugins_registry` in `config.yaml`), with each
/// entry marked as `installed` if it is currently registered via
/// `POST /v1/mcps`. Cached in memory for five minutes; on error the
/// endpoint falls back to the empty catalog with the underlying error.
pub async fn list_plugins_catalog(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let url = state.config.plugins_registry.clone();
    let installed: std::collections::HashSet<String> = state
        .lifecycle
        .plugin_registry
        .list()
        .into_iter()
        .map(|p| p.name)
        .collect();

    let (entries, error) = match url.as_deref() {
        Some(u) => match crate::plugins::Registry::fetch(u).await {
            Ok(reg) => {
                let mut list = serde_json::Map::new();
                for (id, entry) in reg.plugins {
                    let latest = entry
                        .versions
                        .keys()
                        .max()
                        .cloned()
                        .unwrap_or_else(|| "0.0.0".to_string());
                    let info = entry.versions.get(&latest);
                    list.insert(
                        id.clone(),
                        json!({
                            "id": id,
                            "repo": entry.repo,
                            "description": entry.description,
                            "latest_version": latest,
                            "installed": installed.contains(&id),
                            "installed_in_daemon": installed.contains(&id),
                            "artifact": info.map(|v| v.artifact.clone()),
                            "sha256": info.map(|v| v.sha256.clone()),
                        }),
                    );
                }
                (list, None)
            }
            Err(e) => (
                serde_json::Map::new(),
                Some(format!("registry fetch failed: {e}")),
            ),
        },
        None => (
            serde_json::Map::new(),
            Some("no plugins_registry configured".to_string()),
        ),
    };

    Json(json!({
        "installed": installed.into_iter().collect::<Vec<_>>(),
        "registry_url": url,
        "plugins": entries,
        "error": error,
    }))
}

/// DELETE /v1/mcps/:name — deregister an MCP.
///
/// Removes the MCP from the in-memory registry (tools become unavailable
/// to new sessions; existing sessions that already invoked the tool
/// keep working until they finish). Emits `McpUnregistered` via WS so
/// the admin SPA can refresh its listing.
///
/// Useful for testing (remove a smoke-test MCP without restarting the
/// daemon) and as a cleanup path.
pub async fn unregister_mcp(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let removed = state.lifecycle.plugin_registry.remove(&name);
    if !removed {
        return Err((
            StatusCode::NOT_FOUND,
            format!("mcp '{name}' not registered"),
        ));
    }
    state.emit(Event::McpUnregistered { name: name.clone() });
    Ok(Json(json!({ "unregistered": name })))
}

/// POST /v1/mcps/clean — drop every registered MCP.
///
/// Drains the in-memory registry and emits `McpUnregistered` for each.
/// Useful as a single-call "factory reset" for testing. The four real
/// MCPs (memory/llmd/clickup/voice) re-register themselves within 60s.
pub async fn clean_mcps(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let names: Vec<String> = state
        .lifecycle
        .plugin_registry
        .list()
        .into_iter()
        .map(|p| p.name)
        .collect();
    for name in &names {
        state.lifecycle.plugin_registry.remove(name);
        state.emit(Event::McpUnregistered { name: name.clone() });
    }
    Json(json!({ "removed": names.len(), "names": names }))
}

// ─── EP-0017-04: Env management endpoints ───────────────────────────────────

/// PUT /v1/env/:key — set an env var persistently.
///
/// Persists to the daemon env file (`NEUROX_ENV_FILE` or
/// `~/.config/neurox/env`) and applies it to the current process.
/// The value is never stored in the response.
pub async fn put_env_var(
    Path(key): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let value = match body.get("value").and_then(serde_json::Value::as_str) {
        Some(v) => v,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "body must contain a string field `value`".into(),
            ))
        }
    };
    crate::environments::set_env_var(&key, value).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    // EP-0018-05: nudge the env-file watchdog so the next poll picks up
    // the change immediately (without waiting the full 5s interval).
    crate::env_watcher::touch_after_write();
    Ok(Json(json!({
        "ok": true,
        "key": key,
        "persisted": true,
    })))
}

/// GET /v1/env — list env var names from the daemon env file.
///
/// Values are never returned — only `{key, set}` pairs where `set` reflects
/// whether the var is currently present in the process environment.
pub async fn list_env_vars(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let keys = crate::environments::list_env_file_keys().unwrap_or_default();
    let vars: Vec<serde_json::Value> = keys
        .into_iter()
        .map(|k| {
            let set = std::env::var(&k).is_ok();
            json!({"key": k, "set": set})
        })
        .collect();
    Json(json!({"vars": vars, "path": crate::environments::env_file_path().to_string_lossy()}))
}

// ─── EP-0018-05 — Local GGUF model discovery ─────────────────────────────

/// GET /v1/llm/models/local — list GGUF files available in the daemon's
/// models directory. The directory is sourced from `NEUROX_MODELS_DIR`
/// (default `~/models`). The chat-side model selector (EP-0017-03) is a
/// separate concern — this endpoint exists so the ProvidersPanel can show
/// which models are ready to be loaded by each local provider.
pub async fn list_local_models(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let models: Vec<serde_json::Value> = crate::local_models::list_local_gguf()
        .into_iter()
        .map(|m| {
            json!({
                "filename": m.filename,
                "path": m.path,
                "size_bytes": m.size_bytes,
            })
        })
        .collect();
    let dir = crate::local_models::models_dir().map(|p| p.to_string_lossy().to_string());
    Json(json!({
        "models": models,
        "dir": dir,
        "env_var": crate::local_models::MODELS_DIR_ENV,
    }))
}

// ─── EP-0018-06 — Per-model inference config CRUD ─────────────────────────────

/// GET /v1/llm/models/local/:filename/config — read the inference config
/// for a single model. `:filename` is the URL-encoded absolute path of
/// the GGUF (e.g. `/home/.../models/qwen.gguf` → `%2Fhome%2F...`).
/// Returns 200 with the config (possibly empty/default) or 404 if the
/// model does not exist on disk.
pub async fn get_model_config(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // EP-2026-08-19: previously proxied to llmd. Now the engine
    // exposes `get_model_config` directly.
    let path = std::path::PathBuf::from(filename);
    let cfg = state
        .engine
        .get_model_config(&path)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    match cfg {
        Some(c) => Ok(Json(serde_json::to_value(&c).unwrap_or(serde_json::Value::Null))),
        None => Err((
            axum::http::StatusCode::NOT_FOUND,
            format!("model config not found for {}", path.display()),
        )),
    }
}

#[derive(Deserialize)]
pub struct PutModelConfigReq {
    pub config: tools_engine::model_configs::ModelConfig,
}

/// PUT /v1/llm/models/local/:filename/config — upsert the inference
/// config for a single model. Same path encoding as the GET.
pub async fn put_model_config(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
    Json(body): Json<PutModelConfigReq>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // EP-2026-08-19: previously proxied to llmd. Engine-direct now.
    let path = std::path::PathBuf::from(filename);
    let stored = state
        .engine
        .upsert_model_config(&path, &body.config)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(&stored).unwrap_or(serde_json::Value::Null)))
}

/// GET /v1/llm/models/local/configs — list every persisted model
/// config. Useful for the admin ModelsPanel to show which local models
/// have customisation.
pub async fn list_model_configs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let rows = state
        .engine
        .list_model_configs()
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let configs: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| serde_json::to_value(&r).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok(Json(json!({"configs": configs})))
}

// ─── EP-0018-07 — Hugging Face model search ───────────────────────────────

/// GET /v1/llm/models/hf?search=<query> — search Hugging Face for GGUF
/// quantisations. Returns up to 30 results, filtered to models with a
/// `gguf` tag. Public endpoint — no HF token required.
pub async fn search_hf_models(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let query = params.get("search").cloned().unwrap_or_default();
    let results = tools_engine::hf_models::search(&query).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("HF search failed: {e}"),
        )
    })?;
    let models: Vec<serde_json::Value> = results
        .into_iter()
        .map(|m| {
            json!({
                "id": m.id,
                "display_name": m.display_name(),
                "author": m.author,
                "downloads": m.downloads,
                "last_modified": m.last_modified,
                "has_gguf": m.has_gguf,
                "gated": m.gated,
            })
        })
        .collect();
    Ok(Json(json!({
        "query": query,
        "models": models,
    })))
}

// ─── EP-0018-08 — Model downloader ──────────────────────────────────────

#[derive(Deserialize)]
pub struct DownloadModelReq {
    pub repo_id: String,
    pub filename: String,
}

/// POST /v1/llm/models/download — pull a single GGUF file from
/// Hugging Face into `MODELS_DIR`. Blocks until the file lands (or
/// fails). Returns 200 with the resolved path and size, 400 on
/// invalid input, 500 on network/HF errors.
pub async fn download_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DownloadModelReq>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // EP-2026-08-19: previously proxied to llmd. Engine-direct now.
    let v = state
        .engine
        .download_model(body.repo_id, body.filename)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(v))
}

/// F5.4 runtime fix: read the API key for the given provider from
/// the daemon's env. The env_watcher propagates the user's env file
/// into the daemon's process; we forward the relevant key to llmd so
/// the backend can authenticate.
///
/// The mapping is per-provider-id:
/// - minimax       → MINIMAX_API_KEY
/// - anthropic     → ANTHROPIC_API_KEY
/// - openai_compat  → OPENAI_API_KEY (or NEUROX_LLM_API_KEY fallback)
pub fn provider_api_key(provider_id: Option<&str>) -> Option<String> {
    let env_var = provider_id
        .map(|id| match id {
            "minimax" => "MINIMAX_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            _ => "OPENAI_API_KEY",
        })
        .unwrap_or("NEUROX_LLM_API_KEY");
    std::env::var(env_var).ok().or_else(|| std::env::var("NEUROX_LLM_API_KEY").ok())
}

/// Resolved runtime config for a provider (kind + api_key + base_url + model).
/// Used by post_message / post_message_stream to forward the LLM call
/// details to the agent subprocess, which uses them to (re)build its
/// LlmClient when the user picks a different model mid-session.
struct ResolvedProvider {
    kind: tools_engine::backend::LlmProviderKind,
    api_key: String,
    base_url: String,
    model: String,
}

/// Look up the configured provider by id, resolve its api_key from the
/// env var named by `api_key_env`, and bundle everything for the
/// subprocess. Returns `None` if the provider is unknown OR not
/// configured (api_key_env empty / unset).
async fn resolve_provider_runtime(
    state: &AppState,
    provider_id: &str,
    model: &str,
) -> Option<ResolvedProvider> {
    let list = state.engine.list_providers().await.ok()?;
    let cfg = list.into_iter().find(|p| p.id == provider_id)?;
    let api_key = cfg
        .api_key_env
        .as_deref()
        .and_then(|env| std::env::var(env).ok())?;
    Some(ResolvedProvider {
        kind: tools_engine::backend::LlmProviderKind::from_str(cfg.kind.as_str())
            .unwrap_or(tools_engine::backend::LlmProviderKind::OpenaiCompat),
        api_key,
        base_url: cfg.effective_base_url(),
        model: model.to_string(),
    })
}

/// Resolve the (provider_id, model) tuple for a message: prefer what
/// the client sent; otherwise fall back to the daemon's configured
/// default provider / model. Used by post_message + post_message_stream
/// so ChatBubble (which doesn't have a model picker) still works.
async fn resolve_default_model(
    state: &AppState,
    provider_id: Option<&str>,
    model: Option<&str>,
) -> (String, String) {
    let providers = state.engine.list_providers().await.unwrap_or_default();
    let default_pid = if state.config.llm.default_provider.is_empty() {
        "minimax".to_string()
    } else {
        state.config.llm.default_provider.clone()
    };
    let pid = provider_id
        .map(str::to_string)
        .unwrap_or_else(|| default_pid.clone());
    let cfg = providers.into_iter().find(|p| p.id == pid);
    let m = model
        .map(str::to_string)
        .or_else(|| cfg.as_ref().map(|c| c.effective_model()))
        .unwrap_or_else(|| "MiniMax-M3".to_string());
    (pid, m)
}

// ─── EP-frontend-config: write endpoints for skills / tools / agents / env / sandbox ───

/// POST /v1/skills/:name/enable — mark a skill enabled. Idempotent.
/// POST /v1/tools/:name/enable — clear the disabled flag for a tool
/// (which had previously been disabled). Returns 404 if no tool is
/// registered under that name — disallowing operators to "enable a
/// ghost tool" keeps the toggle UI honest.
/// Body for `PATCH /v1/agents/:id`. Every field is optional — only the
/// ones present in the body get applied. `system_prompt` is the
/// headline use case for the frontend ("edit this agent's prompt"
/// from the Settings tab); the rest round out the editable surface
/// so the UI doesn't have to special-case missing fields.
#[derive(Debug, Default, Deserialize)]
pub struct AgentPatch {
    #[serde(default)]
    pub system_prompt: Option<Option<String>>,
    #[serde(default)]
    pub requires_approval: Option<Vec<String>>,
    #[serde(default)]
    pub approval_timeout_secs: Option<u64>,
    #[serde(default)]
    pub tools_allowlist: Option<Option<Vec<String>>>,
    #[serde(default)]
    pub max_sessions: Option<Option<usize>>,
    #[serde(default)]
    pub idle_timeout_secs: Option<u64>,
}

/// `PATCH /v1/agents/:id` — merge `AgentPatch` into the spec for
/// the named agent. Resolves across all three registries
/// (persistent, ephemeral, session_agents) so the frontend can patch
/// whichever kind the user selected without first having to look it
/// up. Returns 404 when no agent matches `id`.
pub async fn patch_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(patch): Json<AgentPatch>,
) -> Result<Json<Agent>, (StatusCode, String)> {
    // Try persistent first (mirrors `get_agent` lookup order).
    if let Some(prev) = state.lifecycle.registry.get_persistent(&id).await {
        let mut next = prev.clone();
        if let Some(sp) = patch.system_prompt {
            next.system_prompt = sp;
        }
        if let Some(v) = patch.requires_approval.clone() {
            next.requires_approval = v;
        }
        if let Some(v) = patch.approval_timeout_secs {
            next.approval_timeout_secs = v;
        }
        let prev = state.lifecycle.registry.update_persistent(&id, next.clone()).await;
        let spec_json = serde_json::to_value(next.redact_secrets())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(Agent {
            r#type: "persistent".to_string(),
            id: Some(id),
            spec: Some(spec_json),
            status: None,
            kind: None,
            command: None,
            args: None,
            idle_timeout_secs: None,
            max_sessions: None,
            tools_allowlist: None,
            system_prompt: next.system_prompt.clone(),
            ok: Some(true),
            previous_exists: Some(prev.is_some()),
        }));
    }
    if let Some(prev) = state.lifecycle.registry.get_ephemeral_template(&id).await {
        let mut next = prev.clone();
        if let Some(v) = patch.requires_approval.clone() {
            next.requires_approval = v;
        }
        let prev = state
            .lifecycle.registry
            .update_ephemeral_template(&id, next.clone())
            .await;
        let spec_json = serde_json::to_value(next.redact_secrets())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(Agent {
            r#type: "ephemeral_template".to_string(),
            id: Some(id),
            spec: Some(spec_json),
            status: None,
            kind: None,
            command: None,
            args: None,
            idle_timeout_secs: None,
            max_sessions: None,
            tools_allowlist: None,
            // EphemeralAgentSpec doesn't carry a system_prompt.
            system_prompt: None,
            ok: Some(true),
            previous_exists: Some(prev.is_some()),
        }));
    }
    if let Some(prev) = state.lifecycle.session_agents.spec(&id).await {
        let mut next = prev.clone();
        if let Some(sp) = patch.system_prompt {
            next.system_prompt = sp;
        }
        if let Some(v) = patch.tools_allowlist {
            next.tools_allowlist = v;
        }
        if let Some(v) = patch.max_sessions {
            next.max_sessions = v;
        }
        if let Some(v) = patch.idle_timeout_secs {
            next.idle_timeout_secs = v;
        }
        let prev = state.lifecycle.session_agents.update_spec(&id, next.clone()).await;
        let spec_json = serde_json::to_value(&next)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(Agent {
            r#type: "in_process".to_string(),
            id: Some(id),
            spec: Some(spec_json),
            status: None,
            kind: None,
            command: None,
            args: None,
            idle_timeout_secs: None,
            max_sessions: None,
            tools_allowlist: None,
            system_prompt: next.system_prompt.clone(),
            ok: Some(true),
            previous_exists: Some(prev.is_some()),
        }));
    }
    Err((
        StatusCode::NOT_FOUND,
        format!("agent not found: {id}"),
    ))
}

/// DELETE /v1/env/:key — remove a daemon env var from the env file
/// and from the current process. Idempotent: returns 200 even if the
/// key wasn't set (the frontend's DELETE shouldn't have to look up
/// the state first).
pub async fn delete_env_var(
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    crate::environments::unset_env_var(&key)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    crate::env_watcher::touch_after_write();
    Ok(Json(json!({
        "ok": true,
        "key": key,
        "persisted": true,
    })))
}


/// EP-2026-08-19: serve a file from the local filesystem so the chat
/// UI can preview media produced by tools (`generate_image`,
/// `generate_music`, `generate_video`). The path is URL-decoded and
/// canonicalized — we reject paths that escape via `..` segments but
/// don't restrict to a specific home dir (the daemon already runs as
/// the operator and $HOME in systemd can be wrong).
///
/// Auth: same Bearer token as other /v1 routes (handled by
/// `AuthLayer` in router/mod.rs).
fn session_not_found_response() -> Response {
    axum::http::Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            r#"{"error":"session not found"}"#,
        ))
        .expect("static response build")
}


pub async fn serve_file(
    State(_state): State<Arc<AppState>>,
    Path(raw_path): Path<PathBuf>,
) -> Result<Response, (StatusCode, String)> {
    use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};

    // Path<PathBuf> is the right extractor for the `*path` wildcard in
    // axum 0.7 — Path<String> would reject paths containing slashes.
    let decoded = match percent_decode_str(&raw_path.to_string_lossy()).decode_utf8() {
        Ok(s) => s.into_owned(),
        Err(_) => return Err((StatusCode::BAD_REQUEST, "non-utf8 path".to_string())),
    };

    // Reject `..` segments (raw check, pre-canonicalization) — catches
    // obvious traversal attempts even if a downstream canonicalize is
    // bypassed by a symlink.
    for seg in decoded.split('/') {
        if seg == ".." {
            return Err((StatusCode::FORBIDDEN, "path traversal".to_string()));
        }
    }

    // Canonicalize (resolves `..`, `.`, symlinks). If the canonical path
    // still contains a `..` component, it's outside the filesystem root
    // and we reject it.
    let raw = PathBuf::from(&decoded);
    let resolved = tokio::fs::canonicalize(&raw).await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => (StatusCode::NOT_FOUND, "file not found".to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("canonicalize: {e}")),
    })?;
    for comp in resolved.components() {
        if let std::path::Component::ParentDir = comp {
            return Err((StatusCode::FORBIDDEN, "path traversal".to_string()));
        }
    }

    // EP-0011 S-006: serve_root whitelist. Only files under these roots
    // are accessible via /v1/files/*. Prevents reading JWT secret,
    // /etc/passwd, or any file outside the daemon's data directories.
    // Expand `~` to $HOME.
    let home = std::env::var("HOME").unwrap_or_default();
    let expand_tilde = |p: &str| -> std::path::PathBuf {
        if let Some(rest) = p.strip_prefix("~/") {
            std::path::PathBuf::from(&home).join(rest)
        } else {
            std::path::PathBuf::from(p)
        }
    };
    let serve_roots: Vec<std::path::PathBuf> = vec![
        expand_tilde("~/.local/share/neurox/output"),
        expand_tilde("~/.local/share/neurox/identity"),
        expand_tilde("~/.local/share/neurox/logs"),
    ];
    let allowed = serve_roots.iter().any(|root| resolved.starts_with(root));
    if !allowed {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "path '{}' outside serve_root whitelist",
                resolved.display()
            ),
        ));
    }

    let bytes = tokio::fs::read(&resolved)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => (StatusCode::NOT_FOUND, "file not found".to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("read: {e}")),
        })?;

    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "txt" | "md" => "text/plain; charset=utf-8",
        "json" => "application/json",
        _ => "application/octet-stream",
    };
    let filename = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let response = (
        StatusCode::OK,
        [
            (CONTENT_TYPE, mime.to_string()),
            (
                CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", filename.replace('"', "_")),
            ),
        ],
        bytes,
    )
        .into_response();
    Ok(response)
}

fn percent_decode_str(s: &str) -> percent_encoding::PercentDecode<'_> {
    percent_encoding::percent_decode_str(s)
}
