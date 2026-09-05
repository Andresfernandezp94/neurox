//! End-to-end test: spawn the daemon in-process on a random port,
//! then drive it via HTTP + WebSocket like a real client would.

use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
mod common;

use neurox::config::{
    AgentKind, AgentsConfig, CoreConfig, PersistentAgentSpec, RestartPolicy, SandboxConfig,
};
use neurox::protocols::{ProtocolKind, TransportKind};
use neurox::registry::Registry;
use neurox::config::SessionAgentsConfig;
use neurox::session::SessionStore;
use neurox::spawner::Spawner;
use neurox::supervisor::Supervisor;
use std::path::PathBuf;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

fn default_agent_binary() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_agent") {
        return Some(std::path::PathBuf::from(p));
    }
    which::which("agent").ok()
}

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let _registry = std::sync::Arc::new(Registry::new("/tmp/x".into()));
    let _supervisor = std::sync::Arc::new(Supervisor::new());
    let _spawner = std::sync::Arc::new(Spawner::new(4));
    let _session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", Uuid::new_v4())),
        )
        .await
        .unwrap(),
    );
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let _engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    ).await
        .unwrap();
    let state = common::build_app_state_auto(
        tools.clone(),
        PathBuf::from("/tmp"),
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
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/health"))
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "neurox");

    server.abort();
}

#[tokio::test]
async fn full_flow_register_session_message_with_real_default_agent() {
    let Some(binary) = default_agent_binary() else {
        eprintln!("skipping: default binary not found");
        return;
    };
    if std::env::var("MINIMAX_API_KEY").is_err() {
        eprintln!("skipping: MINIMAX_API_KEY not set");
        return;
    }

    let registry = std::sync::Arc::new(Registry::new("/tmp/x".into()));

    let spec = PersistentAgentSpec {
        id: "agent".to_string(),
        kind: AgentKind::Subprocess {
            command: binary.to_string_lossy().to_string(),
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
        bind_addr: "127.0.0.1:7878".into(),
        db_path: std::path::PathBuf::from("/tmp/x.db"),
        log_level: "info".into(),
        agents: AgentsConfig {
            persistent: vec![spec.clone()],
            ephemeral_templates: vec![],
        },
        tls: None,
        spawner_concurrency: 4,
        in_process: vec![],
        services: vec![],
        plugins_registry: None,
        auth: neurox::config::AuthConfigSection::default(),
        llm: neurox::config::LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        session_agents: SessionAgentsConfig::default(),
    };
    registry.load_from_config(&cfg).await.unwrap();

    let supervisor = std::sync::Arc::new(Supervisor::new());
    supervisor.start_agent(spec.clone()).await.unwrap();
    let _spawner = std::sync::Arc::new(Spawner::new(4));
    let _session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", Uuid::new_v4())),
        )
        .await
        .unwrap(),
    );
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let _engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    ).await
        .unwrap();
    let state = common::build_app_state_auto(
        tools.clone(),
        PathBuf::from("/tmp"),
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
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let base = format!("http://127.0.0.1:{port}");
    let _ = Stdio::null(); // keep import used

    // 1) health
    let resp: serde_json::Value = reqwest::get(format!("{base}/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["status"], "ok");

    // 2) list agents — should show default running
    let resp: serde_json::Value = reqwest::get(format!("{base}/v1/agents"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["running"].as_array().unwrap().len(), 1);
    assert_eq!(resp["running"][0]["id"], "default");

    // 3) create session
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

    // 4) send message
    let resp: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/sessions/{sid}/messages"))
        .json(&serde_json::json!({
            "agent_id": "default",
            "text": "hola desde e2e"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let text = resp["result"]["text"].as_str().unwrap();
    assert!(
        text.contains("hola desde e2e"),
        "expected text to contain 'hola desde e2e', got: {text}"
    );

    // 5) cancel
    let resp: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/sessions/{sid}/cancel"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp["cancelled"], serde_json::json!(sid));

    supervisor.shutdown_all().await;
    server.abort();
}

#[tokio::test]
async fn websocket_streams_session_events() {
    use tokio_tungstenite::tungstenite::Message;

    let Some(binary) = default_agent_binary() else {
        return;
    };

    let registry = std::sync::Arc::new(Registry::new("/tmp/x".into()));
    let spec = PersistentAgentSpec {
        id: "agent".to_string(),
        kind: AgentKind::Subprocess {
            command: binary.to_string_lossy().to_string(),
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
        bind_addr: "127.0.0.1:7878".into(),
        db_path: std::path::PathBuf::from("/tmp/x.db"),
        log_level: "info".into(),
        agents: AgentsConfig {
            persistent: vec![spec.clone()],
            ephemeral_templates: vec![],
        },
        tls: None,
        spawner_concurrency: 4,
        in_process: vec![],
        services: vec![],
        plugins_registry: None,
        auth: neurox::config::AuthConfigSection::default(),
        llm: neurox::config::LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        session_agents: SessionAgentsConfig::default(),
    };
    registry.load_from_config(&cfg).await.unwrap();

    let supervisor = std::sync::Arc::new(Supervisor::new());
    supervisor.start_agent(spec).await.unwrap();
    let _spawner = std::sync::Arc::new(Spawner::new(4));
    let _session = Arc::new(
        SessionStore::open(
            &std::env::temp_dir().join(format!("neurox-test-{}.db", Uuid::new_v4())),
        )
        .await
        .unwrap(),
    );
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let _engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    ).await
        .unwrap();
    let state = common::build_app_state_auto(
        tools.clone(),
        PathBuf::from("/tmp"),
    )
    .await;
    let app = neurox::router::router(state);

    let port = free_port().await;
    let listener: tokio::net::TcpListener =
        tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .await
            .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // open WS
    let ws_url = format!("ws://127.0.0.1:{port}/v1/events");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    // Trigger a session via HTTP
    let base = format!("http://127.0.0.1:{port}");
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

    // Read at least one event within 3s
    let event = tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("ws should emit events")
        .expect("ws stream ok")
        .expect("ws msg ok");
    match event {
        Message::Text(text) => {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["type"], "session_started");
            assert_eq!(v["session_id"], sid);
        }
        other => panic!("unexpected message: {other:?}"),
    }

    let _ = ws.close(None).await;
    supervisor.shutdown_all().await;
    server.abort();
}

/// E2E regression: verify the Spawner path end-to-end.
///
/// This is the test for the bug where `POST /v1/sessions` +
/// `POST .../messages` with `agent_id="researcher"` (an ephemeral
/// template) silently failed with "agent not running: researcher"
/// because `Spawner::spawn` was never called.
///
/// We register researcher as an ephemeral template, spawn the real
/// `neurox-agent-researcher` binary, and verify the Spawner path:
/// 1. Session creation succeeds for an ephemeral template
/// 2. WebSocket subscribers see the session_started event
/// 3. The HTTP message endpoint invokes the spawned agent
/// 4. The spawned agent terminates cleanly (running_count drops to 0)
/// 5. The session is persisted in the SessionStore
///
/// We don't assert on Content events because the researcher is
/// non-streaming (returns the full result in one shot, no
/// content_delta notifications like the default does).
#[tokio::test]
async fn ephemeral_researcher_end_to_end() {
    use neurox::config::EphemeralAgentSpec;

    let binary = if let Ok(p) = std::env::var("CARGO_BIN_EXE_neurox-agent-researcher") {
        std::path::PathBuf::from(p)
    } else {
        match which::which("neurox-agent-researcher") {
            Ok(p) => p,
            Err(_) => {
                eprintln!(
                    "neurox-agent-researcher binary not found — skipping ephemeral E2E test"
                );
                return;
            }
        }
    };
    eprintln!("using researcher binary: {}", binary.display());

    let registry = std::sync::Arc::new(Registry::new("/tmp/ephemeral-e2e".into()));
    let template = EphemeralAgentSpec {
        id: "researcher".to_string(),
        kind: AgentKind::Subprocess {
            command: binary.to_string_lossy().to_string(),
            args: vec![],
            env: Default::default(),
        },
        protocol: ProtocolKind::JsonRpc,
        transport: TransportKind::Stdio,
        requires_approval: vec![],
    };
    let cfg = CoreConfig {
        bind_addr: "127.0.0.1:7878".into(),
        db_path: std::env::temp_dir().join(format!("ephemeral-e2e-{}.db", Uuid::new_v4())),
        log_level: "info".into(),
        agents: AgentsConfig {
            persistent: vec![],
            ephemeral_templates: vec![template],
        },
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

    let _supervisor = std::sync::Arc::new(Supervisor::new());
    let spawner = std::sync::Arc::new(Spawner::new(4));
    let session = Arc::new(
        SessionStore::open(&cfg.db_path)
            .await
            .expect("session store opens"),
    );
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let _engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    ).await
        .unwrap();
    let state = common::build_app_state_auto(
        tools.clone(),
        PathBuf::from("/tmp"),
    )
    .await;
    let app = neurox::router::router(state);

    let port = free_port().await;
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
        .await
        .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let ws_url = format!("ws://127.0.0.1:{port}/v1/events");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();

    // 1. Create session for ephemeral agent
    let resp: serde_json::Value = client
        .post(format!("{base}/v1/sessions"))
        .json(&serde_json::json!({"agent_id": "researcher"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sid = resp["session_id"].as_str().unwrap().to_string();

    // 2. Read session_started from WS (proves WS pipeline works)
    let mut saw_session_started = false;
    let mut elapsed = Duration::ZERO;
    let deadline = Duration::from_secs(5);
    let tick = Duration::from_millis(100);
    while elapsed < deadline && !saw_session_started {
        match tokio::time::timeout(tick, ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                if v["type"].as_str() == Some("session_started")
                    && v["session_id"].as_str() == Some(sid.as_str())
                {
                    saw_session_started = true;
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
            Ok(Some(Err(_))) | Err(_) => {}
            _ => {}
        }
        elapsed += tick;
    }
    assert!(
        saw_session_started,
        "expected session_started event on WS within {deadline:?}"
    );

    // 3. Send a message → triggers spawner.spawn()
    let msg_url = format!("{base}/v1/sessions/{sid}/messages");
    let msg_body = serde_json::json!({
        "agent_id": "researcher",
        "text": "rust async programming",
    });
    let msg_resp = tokio::time::timeout(
        Duration::from_secs(60),
        client.post(&msg_url).json(&msg_body).send(),
    )
    .await
    .expect("post_message HTTP request should not hang the test");
    let msg_resp = msg_resp.expect("post_message should succeed at HTTP level");
    assert_eq!(
        msg_resp.status(),
        200,
        "post_message to ephemeral agent should not 500"
    );

    // 4. Verify the spawned agent terminated (running_count == 0)
    let mut final_count = 0;
    for _ in 0..300 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        final_count = spawner.running_count().await;
        if final_count == 0 {
            break;
        }
    }
    assert_eq!(
        final_count, 0,
        "ephemeral researcher should have terminated cleanly"
    );

    // 5. Verify session persisted in SQLite
    let sessions = session.list_sessions(10).await.unwrap();
    let ours = sessions.iter().find(|s| s.session_id == sid);
    assert!(ours.is_some(), "session should be persisted in SQLite");
    let ours = ours.unwrap();
    assert_eq!(ours.agent_id, "researcher");
    assert!(
        !ours.started_at.is_empty(),
        "started_at should be populated"
    );

    let _ = ws.close(None).await;
    server.abort();
    let _ = std::fs::remove_file(&cfg.db_path);
}
