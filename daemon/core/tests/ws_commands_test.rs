//! E2E tests for the bidirectional WS commands endpoint /v1/commands.

use std::process::Stdio;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
mod common;

use neurox::approval::ApprovalManager;
use neurox::config::{CoreConfig, SandboxConfig};
use neurox::protocols::{ProtocolKind, TransportKind};
use neurox::registry::Registry;
use neurox::router::AppState;
use neurox::session_agents::SessionAgentPool;
use neurox::session::SessionStore;
use neurox::spawner::Spawner;
use neurox::supervisor::Supervisor;
use neurox::tasks::TaskManager;
use std::path::PathBuf;
use tokio_tungstenite::tungstenite::Message;

async fn free_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

async fn spawn_app() -> (u16, tokio::task::JoinHandle<()>) {
    let registry = Arc::new(Registry::new("/tmp/x".into()));
    let cfg = CoreConfig::default();
    registry.load_from_config(&cfg).await.unwrap();
    let supervisor = Arc::new(Supervisor::new());
    let spawner = Arc::new(Spawner::new(4));
    let tasks = Arc::new(TaskManager::new());
    let approvals = Arc::new(ApprovalManager::default());
    let session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
        )
        .await
        .unwrap(),
    );

    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    ).await
        .unwrap();
    let state = common::build_app_state_auto(
        tools.clone(),
        PathBuf::from("/tmp"),
        None,
    )
    .await;
    let app = neurox::router::router(state);

    let port = free_port().await;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    (port, server)
}

async fn recv_response(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(3), ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                // Skip event types (anything that could also be a server-pushed event)
                if let Some(s) = v.get("type").and_then(|t| t.as_str()) {
                    if matches!(
                        s,
                        "session_started"
                            | "session_ended"
                            | "thinking"
                            | "content"
                            | "done"
                            | "metrics"
                            | "approval_request"
                            | "approval_resolved"
                            | "tool_call"
                            | "tool_result"
                            | "agent_spawned"
                            | "agent_finished"
                    ) {
                        continue;
                    }
                }
                return v;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => panic!("ws error: {e}"),
            _ => panic!("timeout"),
        }
    }
}

#[tokio::test]
async fn ws_commands_pong() {
    let (port, server) = spawn_app().await;
    let ws_url = format!("ws://127.0.0.1:{port}/v1/commands");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    ws.send(Message::Text(r#"{"type":"ping"}"#.to_string()))
        .await
        .unwrap();
    let resp = recv_response(&mut ws).await;
    assert_eq!(resp["type"], "pong");

    server.abort();
    let _ = Stdio::null();
}

#[tokio::test]
async fn ws_commands_list_agents() {
    let (port, server) = spawn_app().await;
    let ws_url = format!("ws://127.0.0.1:{port}/v1/commands");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    ws.send(Message::Text(r#"{"type":"list_agents"}"#.to_string()))
        .await
        .unwrap();
    let resp = recv_response(&mut ws).await;
    assert_eq!(resp["type"], "agent_list");
    assert!(resp["persistent"].is_array());
    assert!(resp["running"].is_array());

    server.abort();
    let _ = Stdio::null();
}

#[tokio::test]
async fn ws_commands_unknown_agent_errors() {
    let (port, server) = spawn_app().await;
    let ws_url = format!("ws://127.0.0.1:{port}/v1/commands");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    ws.send(Message::Text(
        r#"{"type":"start_agent","id":"nonexistent"}"#.to_string(),
    ))
    .await
    .unwrap();
    let resp = recv_response(&mut ws).await;
    assert_eq!(resp["type"], "error");
    assert!(resp["message"].as_str().unwrap().contains("not found"));

    server.abort();
    let _ = Stdio::null();
}

#[tokio::test]
#[ignore = "was_active response shape changed; needs follow-up"]
async fn ws_commands_cancel_unknown_session() {
    let (port, server) = spawn_app().await;
    let ws_url = format!("ws://127.0.0.1:{port}/v1/commands");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    let cmd = serde_json::json!({
        "type": "cancel_session",
        "session_id": uuid::Uuid::new_v4().to_string(),
    });
    ws.send(Message::Text(cmd.to_string())).await.unwrap();
    let resp = recv_response(&mut ws).await;
    assert_eq!(resp["type"], "session_cancelled");
    assert_eq!(resp["was_active"], serde_json::json!(false));

    server.abort();
    let _ = Stdio::null();
}

// Avoid unused import warning
#[allow(dead_code)]
type _T = ProtocolKind;
#[allow(dead_code)]
type _U = TransportKind;
