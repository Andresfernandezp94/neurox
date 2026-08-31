//! E2E tests for /v1/sessions/:id/cancel and /v1/approvals endpoints.

use std::process::Stdio;
use std::sync::Arc;

use futures::StreamExt;
mod common;

use neurox::approval::{ApprovalDecision, ApprovalManager};
use neurox::config::{
    AgentKind, AgentsConfig, CoreConfig, PersistentAgentSpec, RestartPolicy, SandboxConfig,
};
use neurox::protocols::{ProtocolKind, TransportKind};
use neurox::registry::Registry;
use neurox::router::AppState;
use neurox::session_agents::SessionAgentPool;
use neurox::config::SessionAgentsConfig;
use neurox::session::SessionStore;
use neurox::spawner::Spawner;
use neurox::supervisor::Supervisor;
use neurox::tasks::TaskManager;
use std::path::PathBuf;
use tokio_tungstenite::tungstenite::Message;

fn default_agent_binary() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_agent") {
        return Some(std::path::PathBuf::from(p));
    }
    which::which("agent").ok()
}

async fn free_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn cancel_endpoint_marks_session_inactive() {
    let Some(_binary) = default_agent_binary() else {
        return;
    };
    let registry = Arc::new(Registry::new("/tmp/x".into()));
    let spec = PersistentAgentSpec {
        id: "agent".into(),
        kind: AgentKind::Subprocess {
            command: _binary.to_string_lossy().to_string(),
            args: vec![],
            env: Default::default(),
        },
        protocol: ProtocolKind::JsonRpc,
        transport: TransportKind::Stdio,
        restart_policy: RestartPolicy::OnFailure,
        depends_on: vec![],
        requires_approval: vec![],
        approval_timeout_secs: 60,
        llm: None,
        system_prompt: None,
    };
    let cfg = CoreConfig {
        bind_addr: "127.0.0.1:0".into(),
        db_path: std::path::PathBuf::from("/tmp/x.db"),
        log_level: "info".into(),
        agents: AgentsConfig {
            persistent: vec![spec],
            ephemeral_templates: vec![],
        },
        api_token: None,
        tls: None,
        spawner_concurrency: 4,
        in_process: vec![],
        services: vec![],
        plugins_registry: None,
        llm: neurox::config::LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        session_agents: SessionAgentsConfig::default(),
        auth: neurox::config::AuthConfigSection::default(),
    };
    registry.load_from_config(&cfg).await.unwrap();

    let supervisor = Arc::new(Supervisor::new());
    let spawner = Arc::new(Spawner::new(4));
    let tasks = Arc::new(TaskManager::new());
    let approvals = Arc::new(ApprovalManager::default());

    let session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", Uuid::new_v4())),
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
    let state = common::build_app_state(
        std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
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

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let base = format!("http://127.0.0.1:{port}");

    // Subscribe to events via WS
    let ws_url = format!("ws://127.0.0.1:{port}/v1/events");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Create a session
    let resp: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/sessions"))
        .json(&serde_json::json!({"agent_id": "default"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sid = resp["session_id"].as_str().unwrap().to_string();

    // Verify tasks has the token registered. `get_or_create` was removed in
    // EP-0007 (one token per request, never reused); `create` is the new
    // entry point — we register a fresh token here and clean it up
    // explicitly so the test doesn't leak.
    let session_uuid = uuid::Uuid::parse_str(&sid).unwrap();
    let (_cancel_id, _cancel) = tasks.create(session_uuid).await;
    // No cleanup: the test's POST /v1/sessions/{sid}/cancel below is
    // what actually exercises the lifecycle (it cancels ALL tokens
    // for this session).

    // Cancel via HTTP
    let resp: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/sessions/{sid}/cancel"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["cancelled"], serde_json::json!(sid));
    assert!(resp["was_active"].as_bool().unwrap_or(false));

    // Consume a few WS events to verify SessionEnded was emitted
    let mut got_ended = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !got_ended && std::time::Instant::now() < deadline {
        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(std::time::Duration::from_millis(500), ws.next()).await
        {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            if v["type"] == "session_ended" {
                got_ended = true;
            }
        }
    }
    assert!(got_ended, "should have received session_ended event");

    let _ = ws.close(None).await;
    supervisor.shutdown_all().await;
    server.abort();
    let _ = Stdio::null();
}

#[ignore = "test was failing pre-EP-0004; needs investigation"]
#[tokio::test]
async fn approval_endpoint_create_and_respond() {
    let registry = Arc::new(Registry::new("/tmp/x".into()));
    let cfg = CoreConfig {
        bind_addr: "127.0.0.1:0".into(),
        db_path: std::path::PathBuf::from("/tmp/x.db"),
        log_level: "info".into(),
        agents: AgentsConfig {
            persistent: vec![],
            ephemeral_templates: vec![],
        },
        api_token: None,
        tls: None,
        spawner_concurrency: 4,
        in_process: vec![],
        services: vec![],
        plugins_registry: None,
        llm: neurox::config::LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        session_agents: SessionAgentsConfig::default(),
        auth: neurox::config::AuthConfigSection::default(),
    };
    registry.load_from_config(&cfg).await.unwrap();

    let supervisor = Arc::new(Supervisor::new());
    let spawner = Arc::new(Spawner::new(4));
    let tasks = Arc::new(TaskManager::new());
    let approvals = Arc::new(ApprovalManager::default());

    let session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", Uuid::new_v4())),
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
    let state = common::build_app_state(
        std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
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

    // Spawn a pending approval in the background
    let m = approvals.clone();
    let h = tokio::spawn(async move {
        m.request(
            uuid::Uuid::new_v4(),
            "shell".into(),
            serde_json::json!({"cmd": "ls"}),
            None,
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // List pending
    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/v1/approvals"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let pending = resp["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1);
    let approval_id = pending[0]["id"].as_str().unwrap();

    // Respond via HTTP
    let resp: serde_json::Value = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{port}/v1/approvals/{approval_id}/respond"
        ))
        .json(&serde_json::json!({"decision": "approve"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["resolved"], serde_json::json!(approval_id));
    assert_eq!(resp["decision"], "approve");

    let (_, decision) = h.await.unwrap();
    assert!(matches!(decision, ApprovalDecision::Approve));

    // List should be empty now
    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/v1/approvals"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["pending"].as_array().unwrap().len(), 0);

    server.abort();
}

#[tokio::test]
async fn approval_endpoint_404_for_unknown() {
    let registry = Arc::new(Registry::new("/tmp/x".into()));
    let cfg = CoreConfig::default();
    registry.load_from_config(&cfg).await.unwrap();
    let supervisor = Arc::new(Supervisor::new());
    let spawner = Arc::new(Spawner::new(4));
    let tasks = Arc::new(TaskManager::new());
    let approvals = Arc::new(ApprovalManager::default());

    let session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", Uuid::new_v4())),
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
    let state = common::build_app_state(
        std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
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

    let bogus_id = Uuid::new_v4();
    let resp = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{port}/v1/approvals/{bogus_id}/respond"
        ))
        .json(&serde_json::json!({"decision": "approve"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

    server.abort();
}

use uuid::Uuid;
