use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::approval::ApprovalDecision;
use crate::router::http::ClientInfo;
use crate::router::AppState;

/// WebSocket endpoint for streaming events from the core.
/// One-way: core → client (events as text frames).
pub async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    ws.on_upgrade(move |socket| handle_event_socket(socket, state, addr, user_agent))
}

async fn handle_event_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    addr: SocketAddr,
    user_agent: Option<String>,
) {
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

    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let payload = serde_json::to_string(&event).unwrap_or_default();
            if sender.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
    });
    let _ = send_task.await;
    state.events.clients.unregister(&client_id);
}

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
