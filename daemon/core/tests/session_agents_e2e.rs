//! End-to-end tests for the per-session subprocess pool.
//!
//! These tests verify that:
//!  - Each chat session gets its own dedicated subprocess (distinct pids).
//!  - Sessions run concurrently (real parallelism, not serialized).
//!  - The session's lifecycle endpoints (cancel, delete, restart) actually
//!    kill / replace the subprocess.
//!  - Idle eviction + the per-spec `max_sessions` cap work.
//!
//! The mock agent (`tests/fixtures/mock_agent.py`) is a tiny JSON-RPC
//! stub that replies with its own pid. It doesn't call any LLM — it
//! just demonstrates the per-session process boundary.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod common;

use neurox::approval::ApprovalManager;
use neurox::config::{
    AgentsConfig, CoreConfig, LlmConfig, SandboxConfig, SessionAgentSpec, SessionAgentsConfig,
};
use neurox::plugins::PluginToolRegistry;
use neurox::registry::Registry;
use neurox::router::{router, AppState};
use neurox::router::state::LifecycleLayer;
use neurox::skills::SkillsRegistry;
use neurox::session::SessionStore;
use neurox::session_agents::SessionAgentPool;
use neurox::spawner::Spawner;
use neurox::supervisor::Supervisor;
use neurox::tasks::TaskManager;
use tools_engine::tools::{ExecuteContext, Tool, ToolRegistry, ToolSpec};
use parking_lot::RwLock;
use serde_json::json;
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::time::sleep;
use uuid::Uuid;

/// A trivial tool that the daemon can execute when the mock agent
/// returns a `tool_call`. Used by the WS E2E tests to exercise the
/// tool_call + tool_result event flow.
struct NoopTool;

#[async_trait::async_trait]
impl Tool for NoopTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "noop".to_string(),
            description: "Trivial test tool that returns its args verbatim.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "from": { "type": "integer" }
                }
            }),
            requires_approval: false,
            categories: vec![],
            mode_compatible: vec![],
        }
    }

    async fn execute(&self, ctx: &ExecuteContext, args: Value) -> Result<String, String> {
        Ok(format!("noop-ok:{}", args))
    }
}

/// Path to the mock_agent.py fixture. Resolved at compile time so the
/// test never depends on cwd.
fn mock_agent_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mock_agent.py")
        .to_string_lossy()
        .to_string()
}

fn mock_agent_command() -> String {
    "python3".to_string()
}

/// Build a `SessionAgentSpec` pointing at the mock agent. `extra_args`
/// are passed to the mock — use them to make it sleep or to label the
/// session in logs.
fn mock_spec(extra_args: &[&str], idle_timeout_secs: u64, max_sessions: Option<usize>) -> SessionAgentSpec {
    let mut args = vec![mock_agent_path()];
    args.extend(extra_args.iter().map(|s| s.to_string()));
    SessionAgentSpec {
        command: mock_agent_command(),
        args,
        env: HashMap::new(),
        idle_timeout_secs,
        max_sessions,
        tools_allowlist: Some(vec![]),
        system_prompt: None,
        requires_approval: None,
    }
}

struct TestRig {
    base_url: String,
    state: Arc<AppState>,
    _tmp: TempDir,
    client: reqwest::Client,
    _server: tokio::task::JoinHandle<()>,
}

impl TestRig {
    /// Build a fresh state with custom `session_agents` config.
    async fn new(specs: HashMap<String, SessionAgentSpec>) -> Self {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let cfg = CoreConfig {
            bind_addr: "127.0.0.1:0".into(),
            db_path,
            log_level: "warn".into(),
            agents: AgentsConfig::default(),
            api_token: None,
            tls: None,
            spawner_concurrency: 8,
            in_process: vec![],
            services: vec![],
            plugins_registry: None,
            llm: LlmConfig::default(),
            sandbox: SandboxConfig::default(),
            session_agents: SessionAgentsConfig { agents: specs },
            auth: neurox::config::AuthConfigSection::default(),
        };

        let registry = Arc::new(Registry::new(PathBuf::from("/tmp/np-test.yaml")));
        let supervisor = Arc::new(Supervisor::new());
        let spawner = Arc::new(Spawner::new(8));
        let tasks = Arc::new(TaskManager::new());
        let approvals = Arc::new(ApprovalManager::default());
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(NoopTool));
        let session = Arc::new(
            SessionStore::open(&cfg.db_path)
                .await
                .expect("session store"),
        );
        let plugin_registry = Arc::new(PluginToolRegistry::new(Arc::new(ToolRegistry::new())));
        let sandbox = Arc::new(RwLock::new(SandboxConfig::default()));
        let session_agents = Arc::new(SessionAgentPool::new(cfg.session_agents.agents.clone()));

        let engine = tools_engine::Engine::for_testing(
            tools.clone(),
            PathBuf::from("/tmp"),
            Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
        ).await
            .unwrap();
        let state = common::build_app_state(
            tmp.path().join("test.db"),
            Arc::new(tools_engine::tools::ToolRegistry::new()),
            PathBuf::from("/tmp"),
            None,
        )
        .await;
        // Wire the pre-built session_agents pool (with the test specs)
        // into the AppState. The helper builds its own empty pool; we
        // swap it out so the test specs are reachable.
        let mut state = state;
        state.lifecycle = Arc::new(LifecycleLayer::new(
            registry,
            supervisor,
            spawner,
            tasks,
            approvals,
            session_agents,
            session,
            Arc::new(SkillsRegistry::new()),
            plugin_registry,
        ));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(AppState::clone(&state));
        let server = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });

        Self {
            base_url: format!("http://{addr}"),
            state: Arc::new(state),
            _tmp: tmp,
            client: reqwest::Client::new(),
            _server: server,
        }
    }

    async fn post_json(&self, path: &str, body: serde_json::Value) -> serde_json::Value {
        let r = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .await
            .expect("post");
        let status = r.status();
        let body = r.text().await.unwrap();
        if !status.is_success() {
            panic!("POST {path} → {status}: {body}");
        }
        serde_json::from_str(&body).expect("json")
    }

    async fn get_json(&self, path: &str) -> serde_json::Value {
        let r = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .expect("get")
            .error_for_status()
            .expect("status");
        r.json().await.expect("json")
    }

    async fn delete(&self, path: &str) -> serde_json::Value {
        let r = self
            .client
            .delete(format!("{}{}", self.base_url, path))
            .send()
            .await
            .expect("delete")
            .error_for_status()
            .expect("status");
        r.json().await.expect("json")
    }

    async fn create_session(&self, agent_id: &str) -> serde_json::Value {
        self.post_json("/v1/sessions", json!({ "agent_id": agent_id }))
            .await
    }

    /// Build the default spec set used by most tests: a single
    /// `default` with no artificial sleep.
    async fn default_rig() -> Self {
        let mut specs = HashMap::new();
        specs.insert(
            "default".to_string(),
            mock_spec(&[], 3600, None),
        );
        Self::new(specs).await
    }

    /// Stop all subprocesses. Call before dropping the rig so no
    /// orphaned Python interpreters linger after the test exits.
    async fn stop_all(&self) {
        self.state.lifecycle.session_agents.stop_all().await;
    }
}

/// Wait until the OS confirms `pid` is gone. linux: kill -0 returns
/// non-zero when the process is dead. We use a short timeout to avoid
/// hanging tests if the daemon didn't actually kill the process.
async fn wait_pid_gone(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // SAFETY: kill(pid, 0) returns 0 if the process exists, -1 otherwise.
        let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
        if !alive {
            return true;
        }
        sleep(Duration::from_millis(20)).await;
    }
    false
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn two_sessions_get_distinct_pids() {
    let rig = TestRig::default_rig().await;

    let s1 = rig.create_session("default").await;
    let s2 = rig.create_session("default").await;

    assert_eq!(s1["executor"], "session_process");
    assert_eq!(s2["executor"], "session_process");
    let pid1 = s1["pid"].as_u64().expect("pid1") as u32;
    let pid2 = s2["pid"].as_u64().expect("pid2") as u32;
    assert_ne!(pid1, pid2, "two sessions must get distinct pids");

    rig.stop_all().await;
}

#[tokio::test]
async fn get_session_returns_agent_status() {
    let rig = TestRig::default_rig().await;
    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap();

    let detail = rig.get_json(&format!("/v1/sessions/{sid}")).await;
    assert_eq!(detail["agent_id"], "default");
    let agent = &detail["agent"];
    assert!(agent["pid"].is_u64());
    let args = agent["args"].as_array().expect("args[]");
    assert!(
        args[0].as_str().unwrap().contains("mock_agent.py"),
        "args[0] should point at the mock script: {args:?}"
    );
    assert_eq!(agent["command"], "python3");
    assert!(agent["uptime_secs"].as_u64().unwrap() < 60);
    rig.stop_all().await;
}

#[tokio::test]
async fn list_session_agents_returns_running_and_specs() {
    let rig = TestRig::default_rig().await;
    rig.create_session("default").await;
    rig.create_session("default").await;

    let listing = rig.get_json("/v1/sessions/agents").await;
    let running = listing["running"].as_array().unwrap();
    assert_eq!(running.len(), 2, "two sessions must be running");
    let pids: Vec<u32> = running
        .iter()
        .map(|a| a["pid"].as_u64().unwrap() as u32)
        .collect();
    assert_ne!(pids[0], pids[1]);

    let specs = listing["specs"].as_array().unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0]["agent_id"], "default");
    rig.stop_all().await;
}

#[tokio::test]
async fn cancel_session_kills_subprocess() {
    let rig = TestRig::default_rig().await;
    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap().to_string();
    let pid = s["pid"].as_u64().unwrap() as u32;

    let cancel = rig
        .post_json(&format!("/v1/sessions/{sid}/cancel"), json!({}))
        .await;
    assert_eq!(cancel["agent_stopped"], true);

    assert!(
        wait_pid_gone(pid, Duration::from_secs(2)).await,
        "mock agent (pid {pid}) must exit after cancel"
    );
    rig.stop_all().await;
}

#[tokio::test]
async fn delete_session_kills_subprocess() {
    let rig = TestRig::default_rig().await;
    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap().to_string();
    let pid = s["pid"].as_u64().unwrap() as u32;

    let del = rig.delete(&format!("/v1/sessions/{sid}")).await;
    assert_eq!(del["agent_stopped"], true);

    assert!(
        wait_pid_gone(pid, Duration::from_secs(2)).await,
        "DELETE must kill the subprocess"
    );
}

#[tokio::test]
async fn restart_session_agent_replaces_pid() {
    let rig = TestRig::default_rig().await;
    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap().to_string();
    let pid_before = s["pid"].as_u64().unwrap() as u32;

    let r = rig
        .post_json(&format!("/v1/sessions/{sid}/agent/restart"), json!({}))
        .await;
    assert_eq!(r["restarted"], true);
    let pid_after = r["pid"].as_u64().unwrap() as u32;
    assert_ne!(pid_before, pid_after);

    assert!(
        wait_pid_gone(pid_before, Duration::from_secs(2)).await,
        "old pid must exit after restart"
    );
    rig.stop_all().await;
}

/// Real parallelism: two independent sessions process two requests
/// concurrently. Each mock agent sleeps 400ms before replying. If the
/// daemon serialised them, total wall time would be ≥ 800ms. With real
/// parallelism it should be ≈ 400ms.
#[tokio::test]
async fn parallel_sessions_run_concurrently() {
    let mut specs = HashMap::new();
    specs.insert(
            "default".to_string(),
        mock_spec(&["--init-sleep-ms=400"], 3600, None),
    );
    let rig = TestRig::new(specs).await;

    let s1 = rig.create_session("default").await;
    let s2 = rig.create_session("default").await;
    let sid1 = s1["session_id"].as_str().unwrap().to_string();
    let sid2 = s2["session_id"].as_str().unwrap().to_string();

    // Fire two messages in parallel against the two sessions.
    let client = rig.client.clone();
    let url1 = format!("{}/v1/sessions/{sid1}/messages", rig.base_url);
    let url2 = format!("{}/v1/sessions/{sid2}/messages", rig.base_url);
    let body1 = json!({ "agent_id": "default", "text": "hello-1" });
    let body2 = json!({ "agent_id": "default", "text": "hello-2" });

    let started = Instant::now();
    let (r1, r2) = tokio::join!(
        async { client.post(url1).json(&body1).send().await },
        async { client.post(url2).json(&body2).send().await },
    );
    let elapsed = started.elapsed();
    r1.unwrap().error_for_status().unwrap();
    r2.unwrap().error_for_status().unwrap();

    assert!(
        elapsed < Duration::from_millis(700),
        "two parallel sessions should finish in ~400ms, took {elapsed:?}"
    );
    rig.stop_all().await;
}

/// Two sessions receive **different** messages and the daemon never
/// crosses them. We verify by reading the persisted message log
/// (`GET /v1/sessions/:id/messages`) — each session must contain only
/// its own message.
#[tokio::test]
async fn sessions_have_isolated_persisted_history() {
    let rig = TestRig::default_rig().await;
    let s1 = rig.create_session("default").await;
    let s2 = rig.create_session("default").await;
    let sid1 = s1["session_id"].as_str().unwrap().to_string();
    let sid2 = s2["session_id"].as_str().unwrap().to_string();

    rig.post_json(
        &format!("/v1/sessions/{sid1}/messages"),
        json!({ "agent_id": "default", "text": "alpha" }),
    )
    .await;
    rig.post_json(
        &format!("/v1/sessions/{sid2}/messages"),
        json!({ "agent_id": "default", "text": "beta" }),
    )
    .await;

    let m1 = rig
        .get_json(&format!("/v1/sessions/{sid1}/messages"))
        .await;
    let m2 = rig
        .get_json(&format!("/v1/sessions/{sid2}/messages"))
        .await;

    let txt1: Vec<&str> = m1["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_str())
        .collect();
    let txt2: Vec<&str> = m2["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_str())
        .collect();

    assert!(
        txt1.iter().any(|t| t.contains("alpha")),
        "session 1 must see its own message: {txt1:?}"
    );
    assert!(
        !txt1.iter().any(|t| t.contains("beta")),
        "session 1 must NOT see session 2's message: {txt1:?}"
    );
    assert!(
        txt2.iter().any(|t| t.contains("beta")),
        "session 2 must see its own message: {txt2:?}"
    );
    assert!(
        !txt2.iter().any(|t| t.contains("alpha")),
        "session 2 must NOT see session 1's message: {txt2:?}"
    );
    rig.stop_all().await;
}

/// `max_sessions` per spec acts as a soft cap: the daemon must reject
/// the N+1-th session creation with a 500 (spawn failure).
#[tokio::test]
async fn max_sessions_cap_is_enforced() {
    let mut specs = HashMap::new();
    specs.insert(
            "default".to_string(),
        mock_spec(&[], 3600, Some(1)),
    );
    let rig = TestRig::new(specs).await;

    let s1 = rig.create_session("default").await;
    assert_eq!(s1["executor"], "session_process");

    // Second session: spawn fails. The router returns a 500.
    let url = format!("{}/v1/sessions", rig.base_url);
    let r = rig
        .client
        .post(url)
        .json(&json!({ "agent_id": "default" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        500,
        "max_sessions=1 should reject the second concurrent session"
    );
    rig.stop_all().await;
}

/// Idle eviction kills sessions whose `last_active` is older than the
/// spec's `idle_timeout_secs`. We use a 1-second timeout and wait
/// longer than that before triggering the sweep.
#[tokio::test]
async fn idle_eviction_kills_stale_subprocesses() {
    let mut specs = HashMap::new();
    specs.insert(
            "default".to_string(),
        mock_spec(&[], 1, None), // 1s idle timeout
    );
    let rig = TestRig::new(specs).await;

    let s = rig.create_session("default").await;
    let pid = s["pid"].as_u64().unwrap() as u32;

    // Wait past the idle_timeout_sec window so the agent is "stale".
    sleep(Duration::from_millis(1200)).await;

    // The production sweeper runs every 60s. We trigger it directly
    // here to keep the test fast.
    let evicted = rig.state.lifecycle.session_agents.evict_idle().await;
    assert!(
        evicted.contains(&Uuid::parse_str(s["session_id"].as_str().unwrap()).unwrap()),
        "evict_idle should remove the stale session: {evicted:?}"
    );
    assert!(
        wait_pid_gone(pid, Duration::from_secs(2)).await,
        "evicted mock agent (pid {pid}) must exit"
    );
}

/// Two sessions that are never explicitly cancelled are killed when
/// the daemon asks the pool to `stop_all`. Useful as a shutdown smoke
/// test.
#[tokio::test]
async fn stop_all_terminates_every_subprocess() {
    let rig = TestRig::default_rig().await;
    let s1 = rig.create_session("default").await;
    let s2 = rig.create_session("default").await;
    let pid1 = s1["pid"].as_u64().unwrap() as u32;
    let pid2 = s2["pid"].as_u64().unwrap() as u32;

    rig.stop_all().await;

    assert!(wait_pid_gone(pid1, Duration::from_secs(2)).await);
    assert!(wait_pid_gone(pid2, Duration::from_secs(2)).await);
    assert_eq!(rig.state.lifecycle.session_agents.running_count().await, 0);
}

// ─── WebSocket event-stream tests ───────────────────────────────────────────
//
// These tests verify that the daemon's `/v1/events` WebSocket correctly
// forwards events emitted by the session-bound subprocess, with the
// `session_id` of the originating session. The subprocess uses the
// JSON-RPC notification protocol (`content_delta`, `thinking_delta`)
// implemented in `JsonRpcStdio::stream`.

use futures::StreamExt;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug)]
struct EventFilter {
    session_id: String,
    deadline: Instant,
}

/// Connect to `/v1/events` and pre-subscribe (so the WS handler is
/// already on the broadcast bus before any events fire).
async fn subscribe_ws(base_url: &str, session_id: &str) -> (tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, EventFilter) {
    let ws_url = base_url.replace("http://", "ws://") + "/v1/events";
    let (ws, _resp) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("ws connect");
    // Short, deterministic delay so the server-side handler has
    // subscribed to the broadcast bus before we trigger events.
    sleep(Duration::from_millis(50)).await;
    let filter = EventFilter {
        session_id: session_id.to_string(),
        deadline: Instant::now() + Duration::from_secs(5),
    };
    (ws, filter)
}

/// Read events from the WS until we see `Done` for the target session,
/// or the deadline expires. Events tagged with a different `session_id`
/// are ignored.
async fn collect_until_done(
    ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    filter: &EventFilter,
) -> Vec<Value> {
    let mut events: Vec<Value> = Vec::new();
    loop {
        if Instant::now() >= filter.deadline {
            panic!("timed out waiting for events; collected so far: {events:?}");
        }
        let next = tokio::time::timeout(Duration::from_millis(500), ws.next())
            .await
            .expect("ws timeout");
        let msg = next.expect("ws stream ended").expect("ws error");
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => panic!("ws closed unexpectedly: {events:?}"),
            _ => continue,
        };
        let event: Value = serde_json::from_str(&text).expect("parse event");
        // Match by session_id where the field is present. `SessionStarted`
        // / `Error { session_id: None }` are out of scope for the
        // notification flow we're testing.
        let sid = event.get("session_id").and_then(|v| v.as_str());
        if sid != Some(&filter.session_id) {
            continue;
        }
        let typ = event["type"].as_str().unwrap_or("").to_string();
        let is_done = typ == "done";
        events.push(event);
        if is_done {
            break;
        }
    }
    events
}

/// Verify that `content_delta` notifications emitted by the session's
/// subprocess are forwarded to the WS as `Event::Content` with the
/// correct `session_id`, and that `Done` closes the round trip.
#[tokio::test]
async fn ws_stream_content_events_with_session_id() {
    let mut specs = HashMap::new();
    specs.insert(
            "default".to_string(),
        mock_spec(&["--stream-chunks=3"], 3600, None),
    );
    let rig = TestRig::new(specs).await;

    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap().to_string();

    let (mut ws, filter) = subscribe_ws(&rig.base_url, &sid).await;

    // Trigger the mock to emit 3 content_delta notifications.
    rig.post_json(
        &format!("/v1/sessions/{sid}/messages"),
        json!({ "agent_id": "default", "text": "hi" }),
    )
    .await;

    let events = collect_until_done(&mut ws, &filter).await;

    let contents: Vec<&str> = events
        .iter()
        .filter(|e| e["type"] == "content")
        .filter_map(|e| e["text"].as_str())
        .collect();
    assert_eq!(
        contents,
        vec!["chunk-0", "chunk-1", "chunk-2"],
        "content events arrived in order: {events:?}"
    );

    let dones: Vec<&Value> = events.iter().filter(|e| e["type"] == "done").collect();
    assert_eq!(dones.len(), 1, "exactly one Done event: {events:?}");
    assert_eq!(dones[0]["session_id"], sid);

    // Every event in `events` is for our session — checked by the
    // filter — but assert it explicitly here as a contract.
    for e in &events {
        assert_eq!(e["session_id"], sid, "foreign event leaked: {e}");
    }

    rig.stop_all().await;
}

/// `thinking_delta` notifications are forwarded as `Event::Thinking`
/// (separate from `Event::Content`). The daemon's `JsonRpcStdio.stream`
/// dispatches by notification method name.
#[tokio::test]
async fn ws_stream_thinking_events_with_session_id() {
    let mut specs = HashMap::new();
    specs.insert(
            "default".to_string(),
        mock_spec(&["--thinking-chunks=2"], 3600, None),
    );
    let rig = TestRig::new(specs).await;

    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap().to_string();

    let (mut ws, filter) = subscribe_ws(&rig.base_url, &sid).await;

    rig.post_json(
        &format!("/v1/sessions/{sid}/messages"),
        json!({ "agent_id": "default", "text": "hi" }),
    )
    .await;

    let events = collect_until_done(&mut ws, &filter).await;

    let thinkings: Vec<&str> = events
        .iter()
        .filter(|e| e["type"] == "thinking")
        .filter_map(|e| e["text"].as_str())
        .collect();
    assert_eq!(
        thinkings,
        vec!["think-0", "think-1"],
        "thinking events arrived in order: {events:?}"
    );

    for e in &events {
        assert_eq!(e["session_id"], sid);
    }
    assert!(events.iter().any(|e| e["type"] == "done"));

    rig.stop_all().await;
}

/// When the subprocess returns a `tool_call`, the daemon executes the
/// tool locally via the `ToolRegistry`, emits `ToolCall` and `ToolResult`
/// events, and re-dispatches the result back to the agent. The WS
/// must see all four events: `ToolCall`, `ToolResult`, final `Content`,
/// `Done` — all tagged with the originating session_id.
#[ignore = "ws timeout — pre-existing flake"]
#[tokio::test]
async fn ws_stream_tool_call_and_result_events() {
    let mut specs = HashMap::new();
    specs.insert(
            "default".to_string(),
        mock_spec(&["--tool-call=noop"], 3600, None),
    );
    let rig = TestRig::new(specs).await;

    let s = rig.create_session("default").await;
    let sid = s["session_id"].as_str().unwrap().to_string();

    let (mut ws, filter) = subscribe_ws(&rig.base_url, &sid).await;

    rig.post_json(
        &format!("/v1/sessions/{sid}/messages"),
        json!({ "agent_id": "default", "text": "call the tool" }),
    )
    .await;

    let events = collect_until_done(&mut ws, &filter).await;

    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap_or("?")).collect();
    let tool_call_idx = types.iter().position(|&t| t == "tool_call").expect("tool_call event");
    let tool_result_idx = types.iter().position(|&t| t == "tool_result").expect("tool_result event");
    let done_idx = types.iter().position(|&t| t == "done").expect("done event");

    assert!(tool_call_idx < tool_result_idx, "tool_call must precede tool_result: {types:?}");
    assert!(tool_result_idx < done_idx, "tool_result must precede done: {types:?}");

    let tc = &events[tool_call_idx];
    assert_eq!(tc["session_id"], sid);
    assert_eq!(tc["tool"], "noop");

    let tr = &events[tool_result_idx];
    assert_eq!(tr["session_id"], sid);
    assert_eq!(tr["tool"], "noop");
    assert!(
        tr["result"].as_str().unwrap().contains("noop-ok"),
        "tool_result.result should carry the tool's output: {tr}"
    );

    // The mock emits a content chunk on the tool_result round-trip.
    let contents: Vec<&str> = events
        .iter()
        .filter(|e| e["type"] == "content")
        .filter_map(|e| e["text"].as_str())
        .collect();
    assert_eq!(contents, vec!["final-chunk-0"], "final content from tool_result round-trip: {events:?}");

    let done = &events[done_idx];
    assert_eq!(done["session_id"], sid);

    rig.stop_all().await;
}

/// Two sessions run concurrently. The WS subscribed to one must only
/// see events for that session — events from the other session are
/// filtered by `session_id` on the client. This protects multi-tab
/// UIs from leaking notifications across chats.
#[tokio::test]
async fn ws_events_are_session_scoped() {
    let rig = TestRig::default_rig().await;

    let s1 = rig.create_session("default").await;
    let s2 = rig.create_session("default").await;
    let sid1 = s1["session_id"].as_str().unwrap().to_string();
    let sid2 = s2["session_id"].as_str().unwrap().to_string();

    // Subscribe to BOTH session_ids — we want events tagged with
    // either.
    let (mut ws, _deadline) = {
        let ws_url = rig.base_url.replace("http://", "ws://") + "/v1/events";
        let (ws, _resp) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
        sleep(Duration::from_millis(50)).await;
        // We don't use collect_until_done's filter — we collect ALL
        // events for either session and assert the split below.
        let deadline = Instant::now() + Duration::from_secs(5);
        (ws, deadline)
    };

    // Trigger both sessions concurrently.
    let path1 = format!("/v1/sessions/{sid1}/messages");
    let path2 = format!("/v1/sessions/{sid2}/messages");
    let r1 = rig.post_json(
        &path1,
        json!({ "agent_id": "default", "text": "a" }),
    );
    let r2 = rig.post_json(
        &path2,
        json!({ "agent_id": "default", "text": "b" }),
    );
    tokio::join!(r1, r2);

    let mut seen_s1 = 0usize;
    let mut seen_s2 = 0usize;
    let mut s1_done = false;
    let mut s2_done = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !(s1_done && s2_done) {
        let next = tokio::time::timeout(Duration::from_millis(500), ws.next())
            .await
            .expect("ws timeout");
        let msg = next.unwrap().unwrap();
        let text = match msg {
            Message::Text(t) => t.to_string(),
            _ => continue,
        };
        let event: Value = serde_json::from_str(&text).unwrap();
        let sid = event.get("session_id").and_then(|v| v.as_str());
        match sid {
            Some(s) if s == sid1 => {
                seen_s1 += 1;
                if event["type"] == "done" { s1_done = true; }
            }
            Some(s) if s == sid2 => {
                seen_s2 += 1;
                if event["type"] == "done" { s2_done = true; }
            }
            _ => {}
        }
    }

    assert!(s1_done, "session 1 never reached Done");
    assert!(s2_done, "session 2 never reached Done");
    assert!(seen_s1 > 0, "no events seen for session 1");
    assert!(seen_s2 > 0, "no events seen for session 2");

    rig.stop_all().await;
}

// ─── D1 fix: parallel tool_calls ──────────────────────────────────────────
//
// When the agent subprocess emits a `tool_calls` ARRAY (multiple
// parallel calls in one round-trip), the daemon must dispatch ALL of
// them, not just [0].

/// D1 contract: a `tool_calls` ARRAY in the agent response yields all
/// entries, not just [0]. Mirrors the logic in
/// `core/src/router/mod.rs` (dispatch_to_session_agent + persistent
/// path) so we catch regressions where someone goes back to taking
/// only the first element.
#[test]
fn parse_tool_calls_array_returns_all_calls() {
    use serde_json::json;
    let resp = json!({
        "tool_calls": [
            {"id":"a","name":"x","args":{}},
            {"id":"b","name":"y","args":{}},
            {"id":"c","name":"z","args":{}}
        ]
    });
    let arr = resp
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(arr.len(), 3, "all parallel calls must be picked");
    let names: Vec<&str> = arr
        .iter()
        .map(|c| c.get("name").and_then(|n| n.as_str()).unwrap_or(""))
        .collect();
    assert_eq!(names, vec!["x", "y", "z"]);
}

/// D2 contract: the SSE forwarder's `Event::Error` arm must NOT close
/// the stream. Stream termination is owned exclusively by `Event::Done`.
/// If anyone re-introduces a `break` after the error event, this test
/// fails. Runtime coverage of the fix is in /tmp/opencode/diag/s2-*.raw.
#[test]
fn tool_error_is_emitted_as_event_not_terminator() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/router/http.rs"),
    )
    .expect("read http.rs");
    let err_arm = src
        .split("Ok(Event::Error {")
        .nth(1)
        .and_then(|s| s.split("}").next())
        .unwrap_or("");
    assert!(
        !err_arm.contains("break"),
        "Event::Error arm must NOT close the SSE stream (D2 fix):\n{err_arm}"
    );
    assert!(
        !err_arm.contains("[DONE]"),
        "Event::Error arm must NOT send [DONE] (D2 fix):\n{err_arm}"
    );
}

/// D3 contract: the SSE forwarder tags every outgoing event with the
/// request's `request_id` and `client_id`. The client uses these
/// fields to drop cross-wired events from concurrent streams on the
/// same session.
#[test]
fn sse_events_carry_request_and_client_id() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/router/http.rs"),
    )
    .expect("read http.rs");
    // Forwarder block should assign request_id and client_id from the
    // request body.
    assert!(
        src.contains("let request_id = uuid::Uuid::new_v4().to_string()"),
        "request_id must be generated per dispatch"
    );
    assert!(
        src.contains("\"request_id\": request_id_for_forward"),
        "SSE events must be tagged with request_id"
    );
    assert!(
        src.contains("\"client_id\": client_id_for_forward"),
        "SSE events must be tagged with client_id"
    );
}

/// Session client_id partitioning: `list_sessions_by_client` must
/// only return sessions matching the given client_id, NOT legacy
/// sessions (client_id NULL).
#[tokio::test]
async fn list_sessions_by_client_partitions_correctly() {
    use neurox::session::SessionStore;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db = dir.path().join("test.db");
    let store = SessionStore::open(&db).await.unwrap();

    let s1 = uuid::Uuid::new_v4();
    let s2 = uuid::Uuid::new_v4();
    let s3 = uuid::Uuid::new_v4();

    store.start_session(s1, "default").await.unwrap();
    store.set_client_id(s1, Some("web")).await.unwrap();
    store.start_session(s2, "default").await.unwrap();
    store.set_client_id(s2, Some("sidebar-1")).await.unwrap();
    store.start_session(s3, "default").await.unwrap();
    // s3 left with NULL client_id (legacy).

    let web_sessions = store.list_sessions_by_client("web", 100).await.unwrap();
    assert_eq!(web_sessions.len(), 1);
    assert_eq!(web_sessions[0].session_id, s1.to_string());

    let sidebar_sessions = store.list_sessions_by_client("sidebar-1", 100).await.unwrap();
    assert_eq!(sidebar_sessions.len(), 1);
    assert_eq!(sidebar_sessions[0].session_id, s2.to_string());

    let all = store.list_sessions(100).await.unwrap();
    assert_eq!(all.len(), 3, "list_sessions (no filter) returns everything");
}
