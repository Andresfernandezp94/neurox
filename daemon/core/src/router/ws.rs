use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::approval::ApprovalDecision;
use crate::events::Event;
use crate::router::http::ClientInfo;
use crate::router::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct WsAuthQuery {
    /// JWT passed as `?token=…` for WS upgrades (browsers can't set
    /// custom headers in the WS handshake). Authorization header is
    /// also accepted when present.
    #[serde(default)]
    pub token: Option<String>,
}

/// WebSocket endpoint for streaming events from the core.
/// One-way: core → client (events as text frames).
///
/// Authentication: the upgrade request must carry a valid Bearer JWT
/// (Authorization header or `?token=…` query string — browsers don't
/// allow custom headers in the WS handshake). The authenticated
/// `user_id` is then used to filter every emitted event so each
/// device only sees events from sessions owned by its user.
pub async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(auth): Query<WsAuthQuery>,
) -> impl IntoResponse {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    ws.on_upgrade(move |socket| {
        handle_event_socket(socket, state, addr, user_agent, auth_header, auth.token)
    })
}

async fn handle_event_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    addr: SocketAddr,
    user_agent: Option<String>,
    auth_header: Option<String>,
    query_token: Option<String>,
) {
    // Extract Bearer token from header OR `?token=` query string.
    let token = extract_bearer_from_header(auth_header.as_deref()).or(query_token);
    let Some(token) = token else {
        let _ = socket.close().await;
        return;
    };

    // Verify the JWT directly (we can't use the axum extractor here
    // because the upgrade already happened). The same secret is
    // shared with `JwtAuthLayer` so this is the canonical way to
    // validate.
    let claims = match state
        .auth
        .auth
        .as_ref()
        .and_then(|a| crate::auth::middleware::verify_jwt_with(&a.secret, &token).ok())
    {
        Some(c) => c,
        None => {
            let _ = socket.close().await;
            return;
        }
    };
    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
        Ok(u) => u,
        Err(_) => {
            let _ = socket.close().await;
            return;
        }
    };

    let client_id = state.events.clients.register(ClientInfo {
        id: String::new(),
        kind: "ws_events".into(),
        detected_as: ClientInfo::detect(user_agent.as_deref(), "/v1/events"),
        path: "/v1/events".into(),
        method: "GET".into(),
        source_addr: addr.to_string(),
        user_agent,
        connected_at: chrono::Utc::now().to_rfc3339(),
        duration_seconds: 0,
        requests_count: 0,
    });

    let (mut sender, _receiver) = socket.split();
    let mut rx = state.subscribe();
    // `state` is an Arc — clone for the spawned task so we can still
    // unregister the client from the registry after the task ends.
    let state_for_task = Arc::clone(&state);

    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if !event_owned_by(&event, &user_id, &state_for_task).await {
                continue;
            }
            let payload = serde_json::to_string(&event).unwrap_or_default();
            if sender.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
    });
    let _ = send_task.await;
    state.events.clients.unregister(&client_id);
}

/// Decide whether an event belongs to the WS subscriber's user.
/// Three buckets:
///   1. Per-session events (Content, Thinking, Tool*, SessionStarted,
///      SessionEnded, Done, Metrics, MessageAppended, …): look up
///      `session.user_id` and compare.
///   2. Approval events: scope by the request's `session_id`.
///   3. Global events (McpRegistered/Unregistered): emit to everyone.
async fn event_owned_by(
    event: &Event,
    user_id: &uuid::Uuid,
    state: &Arc<AppState>,
) -> bool {
    let session_id = match event {
        Event::SessionStarted { session_id, .. }
        | Event::SessionEnded { session_id, .. }
        | Event::Thinking { session_id, .. }
        | Event::Content { session_id, .. }
        | Event::ToolCall { session_id, .. }
        | Event::ToolResult { session_id, .. }
        | Event::AgentSpawned { session_id, .. }
        | Event::AgentFinished { session_id, .. }
        | Event::Metrics { session_id, .. }
        | Event::Done { session_id, .. }
        | Event::CompactionFailed { session_id, .. } => Some(*session_id),
        Event::ApprovalRequest { request } => Some(request.session_id),
        Event::ApprovalResolved { session_id, .. } => Some(*session_id),
        Event::Error { session_id, .. } => *session_id,
        Event::McpRegistered { .. } | Event::McpUnregistered { .. } => {
            return true;
        }
    };
    let Some(sid) = session_id else {
        return true;
    };
    match state.lifecycle.session.get_session_user(sid).await {
        Ok(Some(owner)) => owner == user_id.to_string(),
        // Legacy orphan session (no user_id) — let the event through.
        // Per-session lifecycle events aren't sensitive enough to leak
        // and the alternative is the tab never closing on the user's
        // own devices.
        Ok(None) => true,
        Ok(Some(_)) => false,
        Err(_) => false,
    }
}

fn extract_bearer_from_header(s: Option<&str>) -> Option<String> {
    s.and_then(|h| h.strip_prefix("Bearer "))
        .map(|t| t.to_string())
}

// Unused helper kept to avoid an unused-import warning if extracted
// helpers are removed. Reserved for future `?token=…` parsing if the
// Query extractor signature changes.
#[allow(dead_code)]
fn _unused_query_collector(_q: &HashMap<String, String>) {}

/// Bidirectional WS endpoint: client sends commands, core responds.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsCommand {
    /// List all registered agents (persistent + ephemeral templates + running)
    ListAgents,
    /// Start a persistent agent by id
    StartAgent { id: String },
    /// Stop a persistent agent by id
    StopAgent { id: String },
    /// Cancel an in-flight session
    CancelSession { session_id: uuid::Uuid },
    /// Respond to a pending approval
    ApprovalResponse {
        id: uuid::Uuid,
        decision: ApprovalDecision,
    },
    /// Heartbeat — core responds with `{"type":"pong"}`
    Ping,
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsResponse {
    AgentList {
        persistent: Vec<serde_json::Value>,
        ephemeral_templates: Vec<serde_json::Value>,
        running: Vec<serde_json::Value>,
    },
    AgentStarted {
        id: String,
        ok: bool,
    },
    AgentStopped {
        id: String,
        ok: bool,
    },
    SessionCancelled {
        session_id: uuid::Uuid,
        was_active: usize,
    },
    ApprovalResolved {
        id: uuid::Uuid,
        decision: String,
    },
    Pong,
    Error {
        message: String,
    },
}

pub async fn ws_commands(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    ws.on_upgrade(move |socket| handle_command_socket(socket, state, addr, user_agent))
}

async fn handle_command_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    addr: SocketAddr,
    user_agent: Option<String>,
) {
    let client_id = state.events.clients.register(ClientInfo {
        id: String::new(),
        kind: "ws_commands".into(),
        detected_as: ClientInfo::detect(user_agent.as_deref(), "/v1/commands"),
        path: "/v1/commands".into(),
        method: "GET".into(),
        source_addr: addr.to_string(),
        user_agent,
        connected_at: chrono::Utc::now().to_rfc3339(),
        duration_seconds: 0,
        requests_count: 0,
    });

    let (mut sender, mut receiver) = socket.split();

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => continue,
        };

        let resp = match serde_json::from_str::<WsCommand>(&text) {
            Ok(cmd) => handle_command(cmd, &state).await,
            Err(e) => WsResponse::Error {
                message: format!("parse error: {e}"),
            },
        };

        let payload = serde_json::to_string(&resp).unwrap_or_default();
        if sender.send(Message::Text(payload)).await.is_err() {
            break;
        }
    }
    state.events.clients.unregister(&client_id);
}

async fn handle_command(cmd: WsCommand, state: &AppState) -> WsResponse {
    match cmd {
        WsCommand::Ping => WsResponse::Pong,
        WsCommand::ListAgents => {
            let persistent = state.lifecycle.registry.list_persistent().await;
            let ephemeral_templates = state.lifecycle.registry.list_ephemeral_templates().await;
            let running = state.lifecycle.supervisor.list().await;
            WsResponse::AgentList {
                persistent: persistent
                    .into_iter()
                    .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
                    .collect(),
                ephemeral_templates: ephemeral_templates
                    .into_iter()
                    .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
                    .collect(),
                running: running
                    .into_iter()
                    .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
                    .collect(),
            }
        }
        WsCommand::StartAgent { id } => match state.lifecycle.registry.get_persistent(&id).await {
            Some(spec) => match state.lifecycle.supervisor.start_agent(spec).await {
                Ok(()) => WsResponse::AgentStarted { id, ok: true },
                Err(e) => WsResponse::Error {
                    message: format!("start failed: {e}"),
                },
            },
            None => WsResponse::Error {
                message: format!("agent not found: {id}"),
            },
        },
        WsCommand::StopAgent { id } => match state.lifecycle.supervisor.stop_agent(&id).await {
            Ok(()) => WsResponse::AgentStopped { id, ok: true },
            Err(e) => WsResponse::Error {
                message: format!("stop failed: {e}"),
            },
        },
        WsCommand::CancelSession { session_id } => {
            let was_active = state.lifecycle.tasks.cancel(session_id).await;
            state.lifecycle.approvals.cancel_session(session_id).await;
            crate::router::AppState::emit_static(
                &state.events.event_tx,
                crate::events::Event::SessionEnded {
                    session_id,
                    summary: Some("cancelled".to_string()),
                },
            );
            WsResponse::SessionCancelled {
                session_id,
                was_active,
            }
        }
        WsCommand::ApprovalResponse { id, decision } => {
            let ok = state.lifecycle.approvals.respond(id, decision).await;
            if ok {
                WsResponse::ApprovalResolved {
                    id,
                    decision: match decision {
                        ApprovalDecision::Approve => "approve".into(),
                        ApprovalDecision::Deny => "deny".into(),
                    },
                }
            } else {
                WsResponse::Error {
                    message: format!("approval not found: {id}"),
                }
            }
        }
    }
}
