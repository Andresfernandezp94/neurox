// Tests de integracion de `/v1/llm/prefs`.
//
// Lo que se verifica aca es el comportamiento de punta a punta del
// endpoint: que la preferencia se persista por usuario, que dos usuarios
// no se pisen, y que un provider sin key se rechace con 400 en vez de
// diferir el error al primer mensaje del chat.
//
// La resolucion pura (`resolve_llm_pref`) ya tiene tests unitarios en
// `tools-engine/src/providers/user_llm_pref.rs`. Estos cubren el cableado
// HTTP: extractor de UserContext, acceso al engine y codigos de estado.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use neurox::auth::{issue_token, AuthState, JwtSecret, ReauthTokens, RefreshStore, Role, UserStore};
use neurox::router::state::AppState;
use tools_engine::config::{LlmProviderConfig, LlmProviderKind};

async fn free_port() -> u16 {
    let l = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

/// `LlmProviderConfig` no implementa `Default` (todos sus campos son
/// parte del contrato), asi que el helper los llena explicitamente.
fn provider(id: &str, model: &str, api_key_env: Option<&str>) -> LlmProviderConfig {
    LlmProviderConfig {
        id: id.to_string(),
        kind: LlmProviderKind::OpenaiCompat,
        base_url: "https://example.invalid/v1".to_string(),
        model: model.to_string(),
        api_key_env: api_key_env.map(str::to_string),
        extra: Default::default(),
        local_command: None,
        local_args: Vec::new(),
        local_model_path: None,
        local_port: None,
    }
}

/// Levanta el daemon en un puerto libre con auth JWT activa, y devuelve
/// (base_url, token, state, secret) para poder registrar providers y
/// mintear un segundo token de otro usuario.
async fn spawn_daemon(tmp: &tempfile::TempDir) -> (String, String, Arc<AppState>, Arc<JwtSecret>) {
    let db_path = tmp.path().join("sessions.db");
    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let mut state = common::build_app_state(db_path, tools, PathBuf::from("/tmp")).await;

    let user_store = Arc::new(UserStore::load(&tmp.path().join("users.json")).unwrap());
    let secret = Arc::new(JwtSecret::generate());
    state.auth = state.auth.with_auth(AuthState {
        user_store,
        secret: secret.clone(),
        expiry_hours: 1,
        reauth_tokens: Arc::new(ReauthTokens::new()),
        refresh_store: Arc::new(RefreshStore::in_memory()),
        refresh_ttl_days: 30,
        google_client_id: String::new(),
        google_client_secret: String::new(),
        google_audience: String::new(),
        cognito_region: String::new(),
        cognito_user_pool_id: String::new(),
        cognito_client_id: String::new(),
    });

    // alpha tiene key; beta es local (sin api_key_env ⇒ siempre
    // configured); gamma declara una env var que NO esta seteada, asi que
    // existe pero no es configured.
    std::env::set_var("NP_TEST_PREF_KEY", "sk-para-test");
    for p in [
        provider("alpha", "alpha-model", Some("NP_TEST_PREF_KEY")),
        provider("beta", "beta-model", None),
        provider("gamma", "gamma-model", Some("NP_TEST_PREF_UNSET_KEY")),
    ] {
        state.engine.create_provider(&p).await.unwrap();
    }
    let token = issue_token(&secret, uuid::Uuid::new_v4(), "tester", Role::Admin, 1).unwrap();
    let app = neurox::router::router(state.clone());
    let port = free_port().await;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    (
        format!("http://127.0.0.1:{port}"),
        token,
        Arc::new(state),
        secret,
    )
}

#[tokio::test]
async fn get_returns_autoconfigured_default_for_fresh_user() {
    // La base del install esta DEFINIDA pero VACIA: un usuario que nunca
    // toco el selector igual obtiene un provider usable, y `explicit`
    // dice que no lo eligio.
    let tmp = tempfile::tempdir().unwrap();
    let (base, token, _state, _secret) = spawn_daemon(&tmp).await;

    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resp["explicit"], serde_json::json!(false));
    // alpha y beta estan configurados; el fallback toma el alfabetico
    // primero para que sea determinista y no dependa del orden de alta.
    assert_eq!(resp["provider_id"], serde_json::json!("alpha"));
    assert_ne!(resp["source"], serde_json::json!("user"));
}

#[tokio::test]
async fn put_persists_and_get_rehydrates() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, token, _state, _secret) = spawn_daemon(&tmp).await;
    let client = reqwest::Client::new();

    let put: serde_json::Value = client
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"provider_id": "beta", "model": "beta-latest"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(put["source"], serde_json::json!("user"));
    assert_eq!(put["explicit"], serde_json::json!(true));

    // GET debe rehidratar: mismo provider/modelo, ahora explicito.
    let get: serde_json::Value = client
        .get(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(get["provider_id"], serde_json::json!("beta"));
    assert_eq!(get["model"], serde_json::json!("beta-latest"));
    assert_eq!(get["source"], serde_json::json!("user"));
    assert_eq!(get["explicit"], serde_json::json!(true));
}

#[tokio::test]
async fn two_users_do_not_overwrite_each_other() {
    // El motivo de existir de la tabla con user_id: con el store global
    // key/value, el ultimo en escribir le ganaba al otro.
    let tmp = tempfile::tempdir().unwrap();
    let (base, token_a, state, secret) = spawn_daemon(&tmp).await;
    let client = reqwest::Client::new();

    // Usuario B, mismo secret de firma pero otro user_id: es lo que
    // hace que la preferencia tenga que estar scopeada por usuario.
    let token_b = issue_token(&secret, uuid::Uuid::new_v4(), "otro", Role::Admin, 1).unwrap();

    client
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token_a)
        .json(&serde_json::json!({"provider_id": "alpha", "model": "a1"}))
        .send()
        .await
        .unwrap();
    client
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token_b)
        .json(&serde_json::json!({"provider_id": "beta", "model": "b1"}))
        .send()
        .await
        .unwrap();

    let a: serde_json::Value = client
        .get(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let b: serde_json::Value = client
        .get(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(a["provider_id"], serde_json::json!("alpha"));
    assert_eq!(a["model"], serde_json::json!("a1"));
    assert_eq!(b["provider_id"], serde_json::json!("beta"));
    assert_eq!(b["model"], serde_json::json!("b1"));

    // Y las filas son distintas en la DB, no una shared.
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT user_id, model FROM user_llm_prefs ORDER BY model")
            .fetch_all(&state.engine.db)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2, "one row per user, not one shared row");
}

#[tokio::test]
async fn put_rejects_provider_without_key() {
    // gamma existe pero no tiene key. Aceptarlo solo difiere el error
    // al primer mensaje del chat.
    let tmp = tempfile::tempdir().unwrap();
    let (base, token, _state, _secret) = spawn_daemon(&tmp).await;

    let status = reqwest::Client::new()
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"provider_id": "gamma"}))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 400, "provider without key must be rejected");
}

#[tokio::test]
async fn put_rejects_unknown_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, token, _state, _secret) = spawn_daemon(&tmp).await;

    let status = reqwest::Client::new()
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"provider_id": "no-existe"}))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 404);
}

#[tokio::test]
async fn put_null_clears_back_to_install_default() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, token, _state, _secret) = spawn_daemon(&tmp).await;
    let client = reqwest::Client::new();

    client
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"provider_id": "beta", "model": "beta-latest"}))
        .send()
        .await
        .unwrap();

    // Clear explicito: vuelve al default del install, no queda en beta.
    let cleared: serde_json::Value = client
        .put(format!("{base}/v1/llm/prefs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"provider_id": null, "model": null}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["explicit"], serde_json::json!(false));
    assert_eq!(cleared["provider_id"], serde_json::json!("alpha"));
}

#[tokio::test]
async fn prefs_require_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, _token, _state, _secret) = spawn_daemon(&tmp).await;

    let status = reqwest::Client::new()
        .get(format!("{base}/v1/llm/prefs"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 401, "prefs is user-scoped, must require auth");
}
