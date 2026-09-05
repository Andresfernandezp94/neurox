// Tests for env management endpoints (EP-0017-04).
//
// - PUT /v1/env/:key — set an env var persistently (file + process)
// - GET /v1/env — list env var names (values never returned)
// - Validation: invalid keys rejected, values never echoed
//
// IMPORTANT: tests never touch the real daemon env file. Each test sets
// `NEUROX_ENV_FILE` to a unique temp path and cleans up afterwards.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::json;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tower::ServiceExt;

mod common;

use neurox::config::CoreConfig;
use neurox::router::router;
use std::path::PathBuf;

/// Global lock serializes integration tests — they share the process-wide
/// `NEUROX_ENV_FILE` var, and cargo runs tests in parallel.
/// Uses `tokio::sync::Mutex` because tests hold the guard across `.await`
/// (clippy rejects `std::sync::MutexGuard` held across await).
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn env_lock() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().await
}

/// Build a router with a fresh AppState (no LLM store needed for env tests).
async fn build_app() -> axum::Router {
    let _registry = Arc::new(neurox::registry::Registry::new(
        std::path::PathBuf::from("/tmp/np-test-env.yaml"),
    ));
    let session_tmp = NamedTempFile::new().unwrap();
    let session_path = session_tmp.path().to_path_buf();
    std::mem::forget(session_tmp);
    let _session = Arc::new(
        neurox::session::SessionStore::open(&session_path)
            .await
            .unwrap(),
    );
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    tools_engine::tools::register_defaults(
        &tools,
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    );

    let _cfg = CoreConfig::default();

    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let _engine = tools_engine::Engine::for_testing(
        tools.clone(),
        PathBuf::from("/tmp"),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    ).await
        .unwrap();
    let state = common::build_app_state(
        std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
        tools.clone(),
        PathBuf::from("/tmp"),
    )
    .await;

    router(state)
}

/// Set the env file path for a test to a unique temp file and return a
/// cleanup guard that removes the env var and any persisted file.
struct EnvGuard {
    path: std::path::PathBuf,
}

fn env_guard(tag: &str) -> EnvGuard {
    let dir = std::env::temp_dir().join(format!("np-env-http-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("env");
    let _ = std::fs::remove_file(&path);
    std::env::set_var("NEUROX_ENV_FILE", &path);
    EnvGuard { path }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        std::env::remove_var("NEUROX_ENV_FILE");
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

#[tokio::test]
async fn put_env_var_persists_and_applies() {
    let _lock = env_lock().await;
    let _g = env_guard("put");
    std::env::remove_var("NP_TEST_KEY");
    let app = build_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/NP_TEST_KEY")
                .header("content-type", "application/json")
                .body(Body::from(json!({"value": "sk-test-123"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["key"], "NP_TEST_KEY");
    // Value is NEVER echoed in the response.
    assert!(v.get("value").is_none());

    // Applied to the current process.
    assert_eq!(
        std::env::var("NP_TEST_KEY").ok().as_deref(),
        Some("sk-test-123")
    );
    std::env::remove_var("NP_TEST_KEY");
}

#[tokio::test]
async fn put_env_var_rejects_invalid_key() {
    let _lock = env_lock().await;
    let _g = env_guard("invalid");
    let app = build_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/not-valid")
                .header("content-type", "application/json")
                .body(Body::from(json!({"value": "x"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn put_env_var_rejects_missing_value() {
    let _lock = env_lock().await;
    let _g = env_guard("noval");
    let app = build_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/NP_MISSING")
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_env_var_lists_names_without_values() {
    let _lock = env_lock().await;
    let _g = env_guard("list");
    let app = build_app().await;

    // Seed the file directly (bypass API for setup).
    std::env::set_var("NP_LISTED_A", "secret-a");
    let seed_path = neurox::environments::env_file_path();
    neurox::environments::merge_env_file(&seed_path, "NP_LISTED_A", "secret-a").unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/env")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let vars = v["vars"].as_array().unwrap();
    let listed = vars.iter().find(|item| item["key"] == "NP_LISTED_A");
    assert!(listed.is_some(), "NP_LISTED_A should be listed");
    // Values never appear anywhere in the response.
    let raw = body.to_vec();
    assert!(
        !raw.windows(9).any(|w| w == b"secret-a"),
        "secret value must not appear in response"
    );
    std::env::remove_var("NP_LISTED_A");
}

#[tokio::test]
async fn get_env_var_empty_when_no_file() {
    let _lock = env_lock().await;
    let _g = env_guard("empty");
    let app = build_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/env")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["vars"].as_array().unwrap().len(), 0);
}
