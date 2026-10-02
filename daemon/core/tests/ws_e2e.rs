//! End-to-end WebSocket test: subscribe to /v1/events, trigger
//! an agent dispatch via HTTP, verify events arrive in real time.

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
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[tokio::test]
async fn ws_stream_emits_session_started_and_done() {
    let Some(binary) = default_agent_binary() else {
        eprintln!("skipping: default binary not found");
        return;
    };
    if std::env::var("MINIMAX_API_KEY").is_err() {
        eprintln!("skipping: MINIMAX_API_KEY not set");
        return;
    }

    // Setup core
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
        llm: neurox::config::LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        workspaces: Default::default(),
        session_agents: SessionAgentsConfig::default(),
        auth: neurox::config::AuthConfigSection::default(),
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
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect WS
    let ws_url = format!("ws://127.0.0.1:{port}/v1/events");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    // Trigger session via HTTP
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

    // First event should be session_started for that sid
    let event = tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("timeout waiting for first event")
        .expect("stream closed")
        .expect("ws error");
    match event {
        Message::Text(t) => {
            let v: serde_json::Value = serde_json::from_str(&t).unwrap();
            assert_eq!(v["type"], "session_started");
            assert_eq!(v["session_id"], sid);
        }
        other => panic!("expected text, got {other:?}"),
    }

    // Now send a message → should get content + done events
    let _ = reqwest::Client::new()
        .post(format!("{base}/v1/sessions/{sid}/messages"))
        .json(&serde_json::json!({"agent_id": "default", "text": "ws streaming test"}))
        .send()
        .await
        .unwrap();

    // Collect events: expect at least content + done within 3s
    let mut got_content = false;
    let mut got_done = false;
    let mut done_text = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline && !(got_content && got_done) {
        let msg = tokio::time::timeout(Duration::from_millis(500), ws.next())
            .await
            .ok()
            .flatten()
            .and_then(std::result::Result::ok)
            .and_then(|m| {
                if let Message::Text(t) = m {
                    Some(t)
                } else {
                    None
                }
            });
        if let Some(t) = msg {
            let v: serde_json::Value = serde_json::from_str(&t).unwrap();
            match v["type"].as_str() {
                Some("content") => {
                    got_content = true;
                }
                Some("done") => {
                    got_done = true;
                    done_text = v["text"].as_str().unwrap_or("").to_string();
                }
                _ => {}
            }
        }
    }

    assert!(got_content, "should have received content event");
    assert!(got_done, "should have received done event");
    assert!(
        done_text.contains("ws streaming test"),
        "done text should echo user input, got: {done_text}"
    );

    let _ = ws.close(None).await;
    supervisor.shutdown_all().await;
    server.abort();
    let _ = Stdio::null();
}
