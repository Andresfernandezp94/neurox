//! Tests for the Bearer auth middleware.
//!
//! Covers:
//! - No token configured: all requests pass through
//! - Token configured: missing header → 401
//! - Token configured: wrong token → 401
//! - Token configured: correct token → 200

use std::time::Duration;

mod common;

use std::sync::Arc;

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

async fn build_app() -> axum::Router {
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let state = common::build_app_state(
        std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
        tools.clone(),
        std::path::PathBuf::from("/tmp"),
    )
    .await;
    neurox::router::router(state)
}

async fn spawn_server(app: axum::Router) -> u16 {
    let port = free_port().await;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    port
}

#[tokio::test]
async fn health_always_public() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    // No auth header — /health should still return 200
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn no_token_configured_allows_everything() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/v1/agents"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn token_configured_missing_header_returns_401() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/v1/agents"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn token_configured_wrong_token_returns_401() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/agents"))
        .header("Authorization", "Bearer wrong-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn token_configured_correct_token_returns_200() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/agents"))
        .header("Authorization", "Bearer secret123")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn token_configured_non_bearer_scheme_returns_401() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/agents"))
        .header("Authorization", "Basic secret123")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
#[ignore = "uses legacy api_token auth removed by EP-2026-08-19 follow-up #7; port to JWT"]
async fn auth_401_body_is_json() {
    let app = build_app().await;
    let port = spawn_server(app).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/v1/agents"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.starts_with("application/json"));
    let body = resp.text().await.unwrap();
    assert!(body.contains("unauthorized"));
}
