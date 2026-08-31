//! End-to-end protocol tests using real subprocesses.
//!
//! Spawns `agent` and exercises `JsonRpcStdio` against it.
//!
//! Tests that require a real LLM call (`process`) are skipped when
//! `MINIMAX_API_KEY` is not set. Protocol-level tests (ping, unknown method)
//! work without the key — the default now starts lazily.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use neurox::protocols::json_rpc_stdio::JsonRpcStdio;
use neurox::protocols::{AgentProtocol, AgentRequest, HealthStatus};

fn default_agent_binary() -> Option<PathBuf> {
    // 1. cargo sets this when running `cargo test` — most reliable
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_agent") {
        return Some(PathBuf::from(p));
    }
    // 2. Prefer the freshly-built binary in target/debug over anything in PATH
    //    (PATH may contain stale installs from previous runs)
    if let Ok(cwd) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(cwd)
            .join("..")
            .join("target")
            .join("debug")
            .join("agent");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    // 3. Last resort: PATH lookup
    which::which("agent").ok()
}

fn has_api_key() -> bool {
    std::env::var("MINIMAX_API_KEY").is_ok()
}

#[tokio::test]
async fn json_rpc_stdio_process_call_succeeds() {
    let Some(binary) = default_agent_binary() else {
        eprintln!("skipping: default binary not found");
        return;
    };
    if !has_api_key() {
        eprintln!("skipping: MINIMAX_API_KEY not set");
        return;
    }

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    let child = cmd.spawn().expect("failed to spawn default agent");
    let protocol = JsonRpcStdio::new(child);

    let req = AgentRequest {
        session_id: uuid::Uuid::new_v4(),
        method: "process".to_string(),
        params: serde_json::json!({"text": "hello from test"}),
    };

    let resp = protocol
        .call(req, tokio_util::sync::CancellationToken::new())
        .await
        .expect("call should not fail at transport level");
    assert!(resp.ok, "response should be ok: {:?}", resp.error);
    let text = resp
        .result
        .as_ref()
        .and_then(|r| r.get("text"))
        .and_then(|v| v.as_str())
        .expect("response should contain text");
    assert!(text.contains("hello from test"));

    let health = protocol.health().await;
    assert_eq!(health.status, HealthStatus::Healthy);

    protocol.shutdown().await.ok();
}

#[tokio::test]
async fn json_rpc_stdio_handles_unknown_method() {
    let Some(binary) = default_agent_binary() else {
        return;
    };
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    let child = cmd.spawn().expect("failed to spawn default agent");
    let protocol = JsonRpcStdio::new(child);

    let req = AgentRequest {
        session_id: uuid::Uuid::new_v4(),
        method: "totally_made_up_method".to_string(),
        params: serde_json::json!({}),
    };

    let resp = protocol
        .call(req, tokio_util::sync::CancellationToken::new())
        .await
        .expect("transport ok");
    assert!(!resp.ok, "unknown method should yield ok=false");
    let err = resp.error.expect("error field should be set");
    assert!(err.contains("unknown method"));
}

#[tokio::test]
async fn json_rpc_stdio_ping_works() {
    let Some(binary) = default_agent_binary() else {
        return;
    };
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    let child = cmd.spawn().unwrap();
    let protocol = JsonRpcStdio::new(child);

    let req = AgentRequest {
        session_id: uuid::Uuid::new_v4(),
        method: "ping".to_string(),
        params: serde_json::json!({}),
    };
    let resp = protocol
        .call(req, tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();
    assert!(resp.ok);
    assert_eq!(
        resp.result.unwrap().get("pong"),
        Some(&serde_json::json!(true))
    );

    protocol.shutdown().await.ok();
}

#[tokio::test]
async fn json_rpc_stdio_multiple_requests_on_same_connection() {
    let Some(binary) = default_agent_binary() else {
        return;
    };
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    let child = cmd.spawn().unwrap();
    let protocol = JsonRpcStdio::new(child);

    for i in 0..5 {
        let req = AgentRequest {
            session_id: uuid::Uuid::new_v4(),
            method: "ping".to_string(),
            params: serde_json::json!({}),
        };
        let resp = tokio::time::timeout(
            Duration::from_secs(3),
            protocol.call(req, tokio_util::sync::CancellationToken::new()),
        )
        .await
        .expect("request should not hang")
        .expect("call ok");
        assert!(resp.ok, "iter {} failed: {:?}", i, resp.error);
    }
    protocol.shutdown().await.ok();
}
