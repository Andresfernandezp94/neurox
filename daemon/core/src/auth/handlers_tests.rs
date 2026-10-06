//! Integration tests for the auth routes.
//!
//! Cubre el ciclo de vida de la sesión tal como lo vive un cliente:
//!
//!   login → refresh token → canjear por JWT → rotar → revocar
//!
//! Estos tests son la razón de existir del refresh token: sin ellos, un
//! fallo en la rotación o en la revocación solo aparecería en producción,
//! cuando ya no hay vuelta atrás. Montan un `AuthState` real sobre un
//! tmpdir, así que ejercitan los handlers de verdad por HTTP en vez de
//! llamar a funciones sueltas.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

use super::handlers::{AuthState, ReauthTokens};
use super::middleware::Role;
use super::tokens::JwtSecret;
use super::users::UserStore;
use crate::auth::refresh::RefreshStore;

const USERNAME: &str = "tester";
const PASSWORD: &str = "correcta-horse-battery";

/// Router de auth sobre un estado efímero, con un usuario `Admin` listo.
/// Devuelve el router y el tmpdir (que debe vivir durante el test).
///
/// `create_user: false` reutiliza el store ya persistido en el tmpdir. Lo
/// necesitan los tests que "rearrancan el daemon" sobre el mismo disco.
async fn build_auth_router(tmp: &TempDir) -> axum::Router {
    build_auth_router_with(tmp, true).await
}

async fn build_auth_router_with(tmp: &TempDir, create_user: bool) -> axum::Router {
    let user_store = UserStore::load(&tmp.path().join("users.json")).expect("user store");
    if create_user {
        user_store
            .create(USERNAME, PASSWORD, Role::Admin)
            .expect("crear usuario");
    }

    let state = AuthState {
        user_store: Arc::new(user_store),
        secret: Arc::new(JwtSecret::generate()),
        // 24 h, igual que tu config.yaml.
        expiry_hours: 24,
        reauth_tokens: Arc::new(ReauthTokens::new()),
        refresh_store: Arc::new(
            RefreshStore::load(&tmp.path().join("refresh_tokens.json")).expect("refresh store"),
        ),
        refresh_ttl_days: 30,
        google_client_id: String::new(),
        google_client_secret: String::new(),
        google_audience: String::new(),
    };

    super::handlers::auth_routes().layer(axum::Extension(state))
}

fn login_request(password: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "username": USERNAME, "password": password }).to_string(),
        ))
        .expect("request")
}

fn refresh_request(token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/auth/refresh-token")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "refresh_token": token, "client": "android" }).to_string(),
        ))
        .expect("request")
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), 8192).await.expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn login_returns_token_and_refresh_token() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let res = app.clone()
        .oneshot(login_request(PASSWORD))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::OK);

    let body = json_body(res).await;
    assert!(body["token"].is_string(), "debe devolver el JWT de acceso");
    assert!(
        body["refresh_token"].is_string(),
        "debe devolver el refresh token; sin él el móvil no puede persistir"
    );
    assert_eq!(body["user"]["username"], USERNAME);
    assert_eq!(body["user"]["role"], "Admin");
}

#[tokio::test]
async fn wrong_password_gets_no_refresh_token() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let res = app.clone()
        .oneshot(login_request("no-es-la-clave"))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let body = json_body(res).await;
    assert!(
        body.get("refresh_token").is_none(),
        "un login fallido no puede emitir refresh token"
    );
}

#[tokio::test]
async fn refresh_token_trades_for_a_new_access_token() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    // Login
    let body = json_body(app.clone().oneshot(login_request(PASSWORD)).await.unwrap()).await;
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    let original_jwt = body["token"].as_str().unwrap().to_string();

    // Canje: sin ninguna sesión previa, solo el refresh token.
    let res = app.clone().oneshot(refresh_request(&refresh)).await.expect("response");
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    assert!(body["token"].is_string(), "debe emitir un JWT nuevo");
    assert_eq!(
        body["user"]["username"], USERNAME,
        "debe devolver el usuario para que el cliente sepa con quién está"
    );
    let rotated = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(rotated, refresh, "el refresh token debe rotar en cada canje");
    // El JWT puede ser byte a byte idéntico: `iat` tiene granularidad de
    // segundo, así que login y canje dentro del mismo segundo producen el
    // mismo token. No es un fallo, pero conviene saberlo para no escribir
    // tests que dependan de que cambie.
    assert_eq!(body["token"].as_str().unwrap(), original_jwt);
}

#[tokio::test]
async fn spent_refresh_token_cannot_be_replayed() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let body = json_body(app.clone().oneshot(login_request(PASSWORD)).await.unwrap()).await;
    let refresh = body["refresh_token"].as_str().unwrap().to_string();

    // Primer canje: vale.
    assert_eq!(
        app.clone().oneshot(refresh_request(&refresh)).await.unwrap().status(),
        StatusCode::OK
    );

    // Segundo canje del mismo token: ya está rotado, no puede servir.
    let res = app.clone()
        .oneshot(refresh_request(&refresh))
        .await
        .expect("response");
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "un refresh token ya usado no puede reutilizarse"
    );
}

#[tokio::test]
async fn garbage_refresh_token_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let res = app.clone()
        .oneshot(refresh_request("inventado"))
        .await
        .expect("response");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoke_invalidates_the_token() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let body = json_body(app.clone().oneshot(login_request(PASSWORD)).await.unwrap()).await;
    let refresh = body["refresh_token"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method("POST")
        .uri("/v1/auth/revoke-token")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "refresh_token": refresh }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::OK, "revocar siempre responde 200");

    // Tras revocar, el token ya no sirve.
    assert_eq!(
        app.clone().oneshot(refresh_request(&refresh)).await.unwrap().status(),
        StatusCode::UNAUTHORIZED,
        "un token revocado no puede canjearse"
    );
}

#[tokio::test]
async fn two_logins_get_independent_refresh_tokens() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let a = json_body(app.clone().oneshot(login_request(PASSWORD)).await.unwrap()).await;
    let b = json_body(app.clone().oneshot(login_request(PASSWORD)).await.unwrap()).await;

    let ta = a["refresh_token"].as_str().unwrap();
    let tb = b["refresh_token"].as_str().unwrap();
    assert_ne!(ta, tb, "cada login emite un token distinto");

    // Revocar uno no debe tumbar al otro.
    let req = Request::builder()
        .method("POST")
        .uri("/v1/auth/revoke-token")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "refresh_token": ta }).to_string(),
        ))
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::OK);

    assert_eq!(
        app.clone().oneshot(refresh_request(tb)).await.unwrap().status(),
        StatusCode::OK,
        "el otro dispositivo debe seguir autenticado"
    );
}

#[tokio::test]
async fn refresh_store_survives_a_restart() {
    let tmp = TempDir::new().unwrap();
    let refresh;

    {
        let app = build_auth_router(&tmp).await;
        let body = json_body(app.clone().oneshot(login_request(PASSWORD)).await.unwrap()).await;
        refresh = body["refresh_token"].as_str().unwrap().to_string();
    }

    // Router nuevo sobre el mismo tmpdir = daemon reiniciado. El usuario ya
    // existe en el store, así que no hay que volver a crearlo.
    let app = build_auth_router_with(&tmp, false).await;
    assert_eq!(
        app.clone().oneshot(refresh_request(&refresh)).await.unwrap().status(),
        StatusCode::OK,
        "el token debe sobrevivir a un reinicio del daemon"
    );
}

#[tokio::test]
async fn google_sign_in_is_404_when_not_configured() {
    let tmp = TempDir::new().unwrap();
    let app = build_auth_router(&tmp).await;

    let req = Request::builder()
        .method("POST")
        .uri("/v1/auth/google")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "id_token": "lo-que-sea" }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.expect("response");

    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "sin client_id el provider debe estar ausente, no fallar de otra forma"
    );
}

#[tokio::test]
async fn google_sign_in_rejects_a_forged_token() {
    let tmp = TempDir::new().unwrap();

    // Daemon con Google configurado, para que el 404 no tape el 401.
    let user_store = UserStore::load(&tmp.path().join("users.json")).unwrap();
    user_store.create(USERNAME, PASSWORD, Role::Admin).unwrap();
    let state = AuthState {
        user_store: Arc::new(user_store),
        secret: Arc::new(JwtSecret::generate()),
        expiry_hours: 24,
        reauth_tokens: Arc::new(ReauthTokens::new()),
        refresh_store: Arc::new(
            RefreshStore::load(&tmp.path().join("refresh_tokens.json")).unwrap(),
        ),
        refresh_ttl_days: 30,
        google_client_id: "123.apps.googleusercontent.com".into(),
        google_client_secret: "secreto".into(),
        google_audience: String::new(),
    };
    let app = super::handlers::auth_routes().layer(axum::Extension(state));

    let req = Request::builder()
        .method("POST")
        .uri("/v1/auth/google")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "id_token": "eyJhbGciOiJub25lIn0.eyJzdWIiOiJhdGFxdWVyIn0." })
                .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.expect("response");

    assert!(
        res.status() == StatusCode::UNAUTHORIZED || res.status() == StatusCode::FORBIDDEN,
        "un ID token falso debe rechazarse, no aceptarse. Status: {}",
        res.status()
    );
}