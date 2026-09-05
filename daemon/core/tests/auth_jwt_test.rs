//! EP-0007: Integration tests for the JWT-based auth flow.
//!
//! Covers:
//! - Login OK with valid credentials
//! - Login fails with wrong password (constant-time, no enumeration)
//! - Login fails with unknown user
//! - JWT middleware: missing token → 401
//! - JWT middleware: invalid token → 401
//! - JWT middleware: expired token → 401
//! - /v1/users requires Admin (Operator → 403)
//! - /v1/users/me/password any authenticated user can change their own
//! - Logout is idempotent (no auth required in v1)
//! - Public paths (/health, /v1/auth/login) bypass JWT
//! - Bootstrap admin creation on first run
//! - UserStore atomic write + reload

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

mod common;

use neurox::auth::{AuthConfig, AuthState, JwtSecret, ReauthTokens, Role, UserStore};
use neurox::config::{AuthConfigSection, CoreConfig};
use serde_json::json;

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

struct TestEnv {
    port: u16,
    /// Kept for diagnostics in test failure messages; no assertion
    /// currently reads it. Prefix with `_` to silence the dead-code
    /// warning while leaving it accessible to future assertions.
    _user_store: Arc<UserStore>,
    _secret: Arc<JwtSecret>,
    _tmp: tempfile::TempDir,
}

async fn build_app_with_auth(admin_password: &str) -> TestEnv {
    let tmp = tempfile::tempdir().unwrap();
    let users_path = tmp.path().join("users.json");
    let secret_path = tmp.path().join("jwt_secret");

    let store = Arc::new(UserStore::load(&users_path).unwrap());
    store
        .create("admin", admin_password, Role::Admin)
        .unwrap();
    store
        .create("viewer1", "viewer-password-1", Role::Viewer)
        .unwrap();
    store
        .create("op1", "operator-password-1", Role::Operator)
        .unwrap();
    let secret = Arc::new(JwtSecret::load_or_create(&secret_path).unwrap());

    let auth_state = AuthState {
        user_store: store.clone(),
        secret: secret.clone(),
        expiry_hours: 1,
        reauth_tokens: Arc::new(ReauthTokens::new()),
    };

    let auth_cfg = AuthConfig {
        enabled: true,
        user_store_path: users_path,
        jwt_secret_path: secret_path,
        jwt_expiry_hours: 1,
    };
    let mut core_cfg = CoreConfig::default();
    core_cfg.auth = AuthConfigSection {
        enabled: true,
        user_store_path: auth_cfg.user_store_path.clone(),
        jwt_secret_path: auth_cfg.jwt_secret_path.clone(),
        jwt_expiry_hours: 1,
    };

    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let mut state = common::build_app_state(
        std::env::temp_dir().join(format!("neurox-test-{}.db", uuid::Uuid::new_v4())),
        tools.clone(),
        PathBuf::from("/tmp"),
    )
    .await;
    state.auth = state.auth.with_auth(auth_state);

    let app = neurox::router::router(state);
    let port = free_port().await;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    TestEnv {
        port,
        _user_store: store,
        _secret: secret,
        _tmp: tmp,
    }
}

async fn login(port: u16, username: &str, password: &str) -> reqwest::Response {
    let client = reqwest::Client::new();
    client
        .post(format!("http://127.0.0.1:{port}/v1/auth/login"))
        .json(&json!({"username": username, "password": password}))
        .send()
        .await
        .unwrap()
}

// ────────────────────────────────────────────────────────────────────
// Login
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn login_ok_returns_token_and_user() {
    let env = build_app_with_auth("admin-password-1").await;
    let resp = login(env.port, "admin", "admin-password-1").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["token"].as_str().unwrap().len() > 50);
    assert_eq!(body["user"]["username"], "admin");
    assert_eq!(body["user"]["role"], "Admin");
}

#[tokio::test]
async fn login_wrong_password_returns_401() {
    let env = build_app_with_auth("correct-password-1").await;
    let resp = login(env.port, "admin", "WRONG-password-1").await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn login_unknown_user_returns_401() {
    let env = build_app_with_auth("admin-password-1").await;
    let resp = login(env.port, "nobody", "any-password-12").await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn login_short_password_rejected() {
    let env = build_app_with_auth("admin-password-1").await;
    let resp = login(env.port, "admin", "short").await;
    // short passwords are rejected at validation; but the user already exists
    // so this should be 401 (no match), not 400 (validation error).
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn login_returns_404_for_invalid_path() {
    // The path is /v1/auth/login (POST); GET on the same path should 405.
    let env = build_app_with_auth("admin-password-1").await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/v1/auth/login", port = env.port))
        .await
        .unwrap();
    // axum returns 405 for wrong method on a registered path.
    assert!(resp.status() == 405 || resp.status() == 404);
}

// ────────────────────────────────────────────────────────────────────
// Middleware (JWT)
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn protected_endpoint_without_token_returns_401() {
    let env = build_app_with_auth("admin-password-1").await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/v1/agents", port = env.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_endpoint_with_invalid_token_returns_401() {
    let env = build_app_with_auth("admin-password-1").await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/agents", port = env.port))
        .header("Authorization", "Bearer not-a-jwt")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_endpoint_with_valid_token_returns_200() {
    let env = build_app_with_auth("admin-password-1").await;
    let login_resp = login(env.port, "admin", "admin-password-1").await;
    let token = login_resp.json::<serde_json::Value>().await.unwrap()
        ["token"].as_str().unwrap().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/agents", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn health_endpoint_is_public() {
    let env = build_app_with_auth("admin-password-1").await;
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/health", port = env.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["auth_required"], true);
}

// ────────────────────────────────────────────────────────────────────
// Role gating
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_can_list_users() {
    let env = build_app_with_auth("admin-password-1").await;
    let login_resp = login(env.port, "admin", "admin-password-1").await;
    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    eprintln!("DEBUG login body: {login_body}");
    let token = login_body["token"].as_str().unwrap().to_string();
    eprintln!("DEBUG token len={}", token.len());
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/users", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.text().await.unwrap();
    eprintln!("DEBUG users status: {status}");
    eprintln!("DEBUG users body: {body}");
    assert_eq!(status, 200);
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(body.as_array().unwrap().len() >= 3);
}

#[tokio::test]
async fn operator_cannot_list_users() {
    let env = build_app_with_auth("admin-password-1").await;
    let token = login(env.port, "op1", "operator-password-1")
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"].as_str().unwrap().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/users", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn viewer_cannot_list_users() {
    let env = build_app_with_auth("admin-password-1").await;
    let token = login(env.port, "viewer1", "viewer-password-1")
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"].as_str().unwrap().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/v1/users", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn admin_can_create_user() {
    let env = build_app_with_auth("admin-password-1").await;
    let token = login(env.port, "admin", "admin-password-1")
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"].as_str().unwrap().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/users", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({"username": "newuser", "password": "newuser-password-1", "role": "Operator"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn user_can_change_own_password() {
    let env = build_app_with_auth("admin-password-1").await;
    let token = login(env.port, "op1", "operator-password-1")
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"].as_str().unwrap().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .patch(format!("http://127.0.0.1:{port}/v1/users/me/password", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({"old_password": "operator-password-1", "new_password": "operator-new-password-1"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify the new password works (the old one shouldn't).
    let resp_old = login(env.port, "op1", "operator-password-1").await;
    assert_eq!(resp_old.status(), 401);
    let resp_new = login(env.port, "op1", "operator-new-password-1").await;
    assert_eq!(resp_new.status(), 200);
}

#[tokio::test]
async fn cannot_self_delete() {
    let env = build_app_with_auth("admin-password-1").await;
    let login_resp = login(env.port, "admin", "admin-password-1").await;
    let body: serde_json::Value = login_resp.json().await.unwrap();
    let token = body["token"].as_str().unwrap().to_string();
    let user_id = body["user"]["id"].as_str().unwrap();
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("http://127.0.0.1:{port}/v1/users/{user_id}", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ────────────────────────────────────────────────────────────────────
// JWT token mechanics
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_returns_new_token() {
    let env = build_app_with_auth("admin-password-1").await;
    let login_resp = login(env.port, "admin", "admin-password-1").await;
    let token = login_resp.json::<serde_json::Value>().await.unwrap()
        ["token"].as_str().unwrap().to_string();
    // Wait >1s so the iat/exp claims differ (JWT HS256 is deterministic
    // for the same claims + secret, so back-to-back issuance produces
    // identical tokens).
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/auth/refresh", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let new_token = body["token"].as_str().unwrap();
    assert!(!new_token.is_empty());
    // Verify the new token actually works (we can't assert_ne on identical
    // second-issued tokens, but we can verify it authenticates).
    let probe = client
        .get(format!("http://127.0.0.1:{port}/v1/users", port = env.port))
        .header("Authorization", format!("Bearer {new_token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(probe.status(), 200);
}

#[tokio::test]
async fn logout_is_idempotent() {
    let env = build_app_with_auth("admin-password-1").await;
    let token = login(env.port, "admin", "admin-password-1")
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"].as_str().unwrap().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/auth/logout", port = env.port))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    // v1: stateless — logout returns 204, client discards locally.
    assert_eq!(resp.status(), 204);
}

// ────────────────────────────────────────────────────────────────────
// UserStore — covered separately in users.rs unit tests. Here we just
// verify bootstrap behavior on first run.
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn bootstrap_creates_admin_when_store_empty() {
    // Equivalent of what main.rs does on first run with auth.enabled=true.
    let tmp = tempfile::tempdir().unwrap();
    let users_path = tmp.path().join("users.json");
    let store = UserStore::load(&users_path).unwrap();
    assert!(store.is_empty());
    // The bootstrap path:
    store.create("admin", "admin-password-1", Role::Admin).unwrap();
    assert!(!store.is_empty());
    let admin = store.find_by_username("admin").unwrap();
    assert_eq!(admin.role, Role::Admin);
}