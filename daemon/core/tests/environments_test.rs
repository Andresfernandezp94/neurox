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
    // NP_LISTED_A no es parte del catalogo, asi que va en `unknown_vars`.
    // Antes aparecia en `vars` porque `vars` era literalmente lo que havia
    // en el archivo.
    let vars = v["vars"].as_array().unwrap();
    assert!(
        !vars.iter().any(|item| item["key"] == "NP_LISTED_A"),
        "catalog vars should only contain known keys"
    );
    let unknown = v["unknown_vars"].as_array().unwrap();
    let listed = unknown.iter().find(|item| item["key"] == "NP_LISTED_A");
    assert!(listed.is_some(), "NP_LISTED_A should be listed as unknown");
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
    // Con el archivo vacio NO hay unknown_vars (nadie escribio nada a
    // mano), pero `vars` trae el catalogo completo: es lo que permite
    // agregar una variable desde la UI.
    assert_eq!(v["unknown_vars"].as_array().unwrap().len(), 0);
    assert!(
        v["vars"].as_array().unwrap().len() > 10,
        "catalog is always returned, even with an empty env file"
    );
}

// ─── Catálogo de env vars (EP-2026-10) ───────────────────────────────────
//
// GET /v1/env antes devolvía solo las keys ya escritas en el archivo, lo
// que hacía imposible agregar una variable desde la UI: no aparecía nada
// que nadie hubiera puesto a mano. Ahora devuelve el catálogo completo con
// metadatos, y el PUT rechaza escribir las de solo lectura.

async fn get_env_json(app: &axum::Router) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(Request::builder().uri("/v1/env").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn get_env_returns_the_catalog_not_only_written_keys() {
    let _g = env_lock().await;
    let _guard = env_guard("catalog");
    let app = build_app().await;

    let v = get_env_json(&app).await;
    let vars = v["vars"].as_array().unwrap();

    // El archivo está vacío, y aun así el catálogo trae variables: es lo
    // que permite agregarlas desde la UI.
    assert!(
        vars.len() > 10,
        "catalog should list known vars even with an empty env file, got {}",
        vars.len()
    );

    // Ninguna debe traer el valor, ni siquiera las no sensibles.
    for var in vars {
        assert!(var.get("value").is_none(), "{} leaked its value", var["key"]);
        assert!(var["key"].is_string());
        assert!(var["category"].is_string());
        assert!(var["description"].as_str().unwrap().len() > 3);
    }
}

#[tokio::test]
async fn catalog_marks_provider_keys_read_only_and_sensitive() {
    let _g = env_lock().await;
    let _guard = env_guard("provkeys");
    let app = build_app().await;

    let v = get_env_json(&app).await;
    let find = |k: &str| {
        v["vars"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["key"] == k)
            .cloned()
            .unwrap_or_else(|| panic!("{k} missing from catalog"))
    };

    for key in ["MINIMAX_API_KEY", "ANTHROPIC_API_KEY", "OPENCODE_API_KEY"] {
        let s = find(key);
        assert_eq!(s["read_only"], true, "{key} must be read-only");
        assert_eq!(s["sensitive"], true, "{key} must be sensitive");
    }

    // Y una var normal sí es editable.
    assert_eq!(find("NEUROX_TODO_DIR")["read_only"], false);
}

#[tokio::test]
async fn catalog_covers_all_categories() {
    let _g = env_lock().await;
    let _guard = env_guard("cats");
    let app = build_app().await;

    let v = get_env_json(&app).await;
    let cats: std::collections::HashSet<String> = v["vars"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["category"].as_str().unwrap().to_string())
        .collect();

    for expected in ["runtime", "default-llm", "auth", "integrations", "infrastructure"] {
        assert!(cats.contains(expected), "missing category {expected}");
    }
    // El bloque de categorías del response, para que el front pueda
    // rotular sin duplicar los nombres.
    assert!(v["categories"].as_array().unwrap().len() >= 5);
}

#[tokio::test]
async fn put_accepts_provider_key_because_the_providers_tab_writes_it() {
    let _g = env_lock().await;
    let _guard = env_guard("provkey-put");
    let app = build_app().await;

    // El catálogo marca las keys de providers `read_only` para que la tab
    // Environment NO ofrezca un editor. Eso es una señal de UI, no una
    // prohibición de escritura: la tab Providers guarda la key por este
    // mismo endpoint (`putEnvVar(api_key_env)`).
    //
    // Este test falló al revés cuando el PUT rechazaba cualquier var
    // `read_only`: guardar una key desde Providers daba 403.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/MINIMAX_API_KEY")
                .header("content-type", "application/json")
                .body(Body::from(json!({"value": "sk-real"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn catalog_marks_provider_keys_not_editable_in_the_env_tab() {
    let _g = env_lock().await;
    let _guard = env_guard("provkey-flag");
    let app = build_app().await;

    // Lo que sí tiene que ser cierto: la UI de Environment no ofrece
    // editor para ellas, aunque la API sí acepte la escritura.
    let resp = app
        .oneshot(Request::builder().uri("/v1/env").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let key = v["vars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == "MINIMAX_API_KEY")
        .unwrap();
    assert_eq!(key["read_only"], true);
    assert_eq!(key["sensitive"], true);
}

#[tokio::test]
async fn put_rejects_read_only_infrastructure_var() {
    let _g = env_lock().await;
    let _guard = env_guard("ro-path");
    let app = build_app().await;

    // Escribir PATH desde la UI tendría efecto hasta el próximo reinicio y
    // después perdería contra el entorno real del host.
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/PATH")
                .header("content-type", "application/json")
                .body(Body::from(json!({"value": "/tmp/evil"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn put_still_accepts_catalog_var() {
    let _g = env_lock().await;
    let _guard = env_guard("ok-put");
    let app = build_app().await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/NEUROX_TODO_DIR")
                .header("content-type", "application/json")
                .body(Body::from(json!({"value": "/tmp/todos"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Y el catálogo lo refleja: ya no está "unset".
    let v = get_env_json(&app).await;
    let todo = v["vars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == "NEUROX_TODO_DIR")
        .unwrap();
    assert_eq!(todo["set"], true);
    assert_eq!(todo["in_file"], true);
}

#[tokio::test]
async fn unknown_vars_are_listed_separately() {
    let _g = env_lock().await;
    let _guard = env_guard("unknown");
    let app = build_app().await;

    // Alguien puso una variable a mano que neurox no conoce.
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/v1/env/MI_VAR_CASERA")
                .header("content-type", "application/json")
                .body(Body::from(json!({"value": "x"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let v = get_env_json(&app).await;
    let unknown = v["unknown_vars"].as_array().unwrap();
    let found = unknown.iter().any(|x| x["key"] == "MI_VAR_CASERA");
    assert!(found, "hand-written var should be listed as unknown");
    assert_eq!(unknown[0]["category"], "custom");
}
