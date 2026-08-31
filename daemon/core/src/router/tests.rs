//! Integration tests for the router (EP-0023-01).
//!
//! Validates that the daemon is API-only (no static serving):
//!   - `/health` returns JSON
//!   - `/v1/tools` works without auth when when
//!   - `/v1/tools` returns a non-empty list with the `shell` tool
//!   - `/v1/tools` requires Bearer token when `api_token` is set
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::approval::ApprovalManager;
use crate::config::{CoreConfig, SandboxConfig};
use crate::registry::Registry;
use crate::router::state::{
    AppState, AuthLayer, EventsLayer, LifecycleLayer, WorkspaceLayer,
};
use crate::router::{router, AppState as _};
use crate::session_agents::SessionAgentPool;
use crate::session::SessionStore;
use crate::spawner::Spawner;
use crate::supervisor::Supervisor;
use crate::tasks::TaskManager;
use tokio::sync::RwLock;

/// Build a minimal `AppState` for tests. Uses tmp paths for any state that
/// needs persistence. Returns `(AppState, TempDir)`.
async fn build_test_state(api_token: Option<&str>) -> (AppState, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("test.db");

    let registry = Arc::new(Registry::new(PathBuf::from("/tmp")));

    let session = Arc::new(
        SessionStore::open(&db_path)
            .await
            .expect("session store open"),
    );

    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let sandbox: Arc<tokio::sync::RwLock<Box<dyn tools_engine::SandboxConfig>>> = Arc::new(
        tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>),
    );
    tools_engine::tools::register_defaults(
        &tools,
        PathBuf::from("/tmp"),
        sandbox.clone(),
    );

    let engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        sandbox,
    )
    .await
    .expect("engine for testing");

    let (event_tx, _) = tokio::sync::broadcast::channel(1024);

    let lifecycle = Arc::new(LifecycleLayer::new(
        registry,
        Arc::new(Supervisor::new()),
        Arc::new(Spawner::new(8)),
        Arc::new(TaskManager::new()),
        Arc::new(ApprovalManager::default()),
        Arc::new(SessionAgentPool::default()),
        session,
        Arc::new(crate::skills::SkillsRegistry::new()),
        Arc::new(crate::plugins::PluginToolRegistry::new(tools)),
    ));

    let auth = AuthLayer::new(api_token.map(|s| s.to_string()));
    let workspace_sandbox: Arc<tokio::sync::RwLock<Box<dyn tools_engine::SandboxConfig>>> = Arc::new(
        tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>),
    );
    let workspace = Arc::new(WorkspaceLayer::new(
        PathBuf::from("/tmp"),
        workspace_sandbox,
    ));

    let state = AppState::new(
        lifecycle,
        Arc::new(EventsLayer::new(event_tx)),
        engine,
        auth,
        workspace,
        Arc::new(CoreConfig::default()),
    );
    (state, tmp)
}

#[tokio::test]
async fn root_path_returns_404_since_daemon_is_api_only() {
    // standalone `neurox-mcp-gui` plugin on its own port.
    let (state, _tmp) = build_test_state(None).await;
    let app = router(state);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn health_returns_json_even_when_root_returns_404() {
    let (state, _tmp) = build_test_state(None).await;
    let app = router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json",
        "/health must return JSON, not the static HTML fallback"
    );
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("body bytes");
    let body_str = std::str::from_utf8(&body).expect("utf8");
    assert!(
        body_str.contains("neurox"),
        "body should be /health JSON, got: {body_str}"
    );
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn v1_tools_endpoint_returns_list_without_auth() {
    let (state, _tmp) = build_test_state(None).await;
    let app = router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 32_768)
        .await
        .expect("body bytes");
    let body_str = std::str::from_utf8(&body).expect("utf8");
    let json: serde_json::Value = serde_json::from_str(body_str).expect("parse JSON");
    let tools = json["tools"].as_array().expect("tools array");
    assert!(!tools.is_empty(), "tools list must not be empty");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert!(
        names.contains(&"shell"),
        "tools list must include 'shell', got: {names:?}"
    );
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn v1_tools_endpoint_with_auth_requires_token() {
    let (state, _tmp) = build_test_state(Some("secret123")).await;
    let app = router(state);

    // Without token: should be rejected by AuthLayer.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response without token");
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "no token => 401"
    );

    // With token: 200 OK.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/tools")
                .header("authorization", "Bearer secret123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response with token");
    assert_eq!(response.status(), StatusCode::OK, "with valid token => 200");
}