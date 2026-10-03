//! Integration tests for the router (EP-0023-01).
//!
//! Validates that the daemon is API-only (no static serving):
//!   - `/health` returns JSON
//!   - root path returns 404
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::approval::ApprovalManager;
use crate::config::CoreConfig;
use crate::registry::Registry;
use crate::router::state::{
    AppState, AuthLayer, EventsLayer, LifecycleLayer, WorkspaceLayer,
};
use crate::router::router;
use crate::session_agents::SessionAgentPool;
use crate::session::SessionStore;
use crate::spawner::Spawner;
use crate::supervisor::Supervisor;
use crate::tasks::TaskManager;

/// Build a minimal `AppState` for tests. Uses tmp paths for any state that
/// needs persistence. Returns `(AppState, TempDir)`.
async fn build_test_state() -> (AppState, TempDir) {
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
    ));

    let auth = AuthLayer::new();
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
    // standalone admin GUI on its own port.
    let (state, _tmp) = build_test_state().await;
    let app = router(state);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn health_returns_json_even_when_root_returns_404() {
    let (state, _tmp) = build_test_state().await;
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
/// EP-2026-10-03: tests de la fase 2 del despacho por lotes.
///
/// Montar un `AppState` real exige un agente subprocess, asi que lo que se
/// prueba aqui es la parte que se puede aislar: que las tools del lote se
/// ejecuten de verdad a la vez, y que el orden de entrega sea el del modelo
/// y no el de completacion.
mod parallel_batch {
    // `super` aqui es el modulo `tests`, no `router`: los items privados
    // del despacho hay que trayirlos desde el ancestro.
    use super::super::{exec_batch_parallel, PreparedToolCall};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::sync::Arc as StdArc;
    use std::time::Duration;
    use tools_engine::{ExecuteContext, Tool, ToolSpec};
    use crate::router::ToolCall;

    /// Tool que tarda `ms` en responder y luego devuelve su etiqueta.
    /// Registra tambien en que instante termino, para poder distinguir
    /// "solapadas" de "una tardia arrastro a la otra".
    struct SlowTool {
        name: String,
        ms: u64,
        concurrent: StdArc<AtomicUsize>,
        peak: StdArc<AtomicUsize>,
    }

    impl SlowTool {
        fn new(name: &str, ms: u64, concurrent: StdArc<AtomicUsize>, peak: StdArc<AtomicUsize>) -> Arc<dyn Tool> {
            Arc::new(Self { name: name.into(), ms, concurrent, peak })
        }
    }

    #[async_trait::async_trait]
    impl Tool for SlowTool {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: self.name.clone(),
                description: "test double".into(),
                parameters: serde_json::json!({"type": "object"}),
                ..Default::default()
            }
        }
        async fn execute(&self, _ctx: &ExecuteContext, args: serde_json::Value) -> Result<String, String> {
            // +1 antes de dormir: mide overlap real, no de planificacion.
            let now = self.concurrent.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(self.ms)).await;
            self.concurrent.fetch_sub(1, Ordering::SeqCst);
            Ok(args.get("tag").and_then(|v| v.as_str()).unwrap_or("?").to_string())
        }
    }

    fn prepared(name: &str, tool: Arc<dyn Tool>, tag: &str) -> PreparedToolCall {
        PreparedToolCall {
            call: ToolCall {
                id: format!("call_{tag}"),
                name: name.to_string(),
                args: serde_json::json!({"tag": tag}),
            },
            tool: Some(tool),
            output: String::new(),
        }
    }

    /// Entrada que ya tiene resultado sin ejecutar (tool denegada o
    /// inexistente): la fase 2 no la toca pero conserva su hueco.
    fn pre_resolved(tag: &str, output: &str) -> PreparedToolCall {
        PreparedToolCall {
            call: ToolCall {
                id: format!("call_{tag}"),
                name: "inventada".to_string(),
                args: serde_json::json!({"tag": tag}),
            },
            tool: None,
            output: output.to_string(),
        }
    }

    fn ctx() -> ExecuteContext {
        ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None }
    }

    /// El test que respalda el cambio: 4 tools de 250ms tardan ~250ms, no
    /// ~1000ms. El margen (900ms) deja sitio para un CI lento sin
    /// convertirlo en un test intermitente que nunca falla por lo que
    /// pretende comprobar.
    #[tokio::test]
    async fn las_tools_del_lote_se_solapan_en_el_tiempo() {
        let concurrent = StdArc::new(AtomicUsize::new(0));
        let peak = StdArc::new(AtomicUsize::new(0));
        let mk = |n: &str| SlowTool::new(n, 250, concurrent.clone(), peak.clone());

        let pending: Vec<PreparedToolCall> = ["a", "b", "c", "d"]
            .iter()
            .map(|t| prepared("t", mk("t"), t))
            .collect();

        let (out, elapsed) = exec_batch_parallel(&ctx(), &pending).await;

        assert_eq!(out.len(), 4, "un resultado por tool");
        for o in &out {
            assert!(o.is_some(), "toda tool del lote devuelve resultado");
        }
        assert_eq!(
            peak.load(Ordering::SeqCst),
            4,
            "las 4 tools estaban en vuelo a la vez"
        );
        assert!(
            elapsed < Duration::from_millis(900),
            "el lote tardo {elapsed:?}: si fuera secuencial serian ~1000ms, \
             asi que las tools NO se estan solapando"
        );
    }

    /// El orden de entrega es el del modelo, no el de completacion: el
    /// watcher del cliente empareja `call_i` con `result_i` por posicion.
    #[tokio::test]
    async fn el_orden_de_entrega_es_el_del_lote_no_el_de_completacion() {
        let concurrent = StdArc::new(AtomicUsize::new(0));
        let peak = StdArc::new(AtomicUsize::new(0));

        // La primera es la mas lenta: si se respeta el orden del modelo,
        // llega la primera; si se respeta el de completacion, la ultima.
        let pending: Vec<PreparedToolCall> = vec![
            prepared("t", SlowTool::new("slow", 400, concurrent.clone(), peak.clone()), "primera"),
            prepared("t", SlowTool::new("fast", 10, concurrent.clone(), peak.clone()), "segunda"),
            prepared("t", SlowTool::new("fast", 10, concurrent.clone(), peak.clone()), "tercera"),
        ];

        let (out, _elapsed) = exec_batch_parallel(&ctx(), &pending).await;

        let tags: Vec<String> = out
            .into_iter()
            .map(|o| o.expect("resultado").expect("sin error"))
            .collect();
        assert_eq!(
            tags,
            vec!["primera", "segunda", "tercera"],
            "los resultados deben llegar en el orden del modelo"
        );
    }

    /// `join_all`, no `try_join_all`: que una tool falle no puede cancelar
    /// a las demas, o el LLM se queda sin ver el resultado que si importa.
    #[tokio::test]
    async fn una_tool_que_falla_no_arrastra_a_las_demas() {
        struct Failing;
        #[async_trait::async_trait]
        impl Tool for Failing {
            fn spec(&self) -> ToolSpec {
                ToolSpec { name: "boom".into(), description: String::new(), parameters: serde_json::json!({}), ..Default::default() }
            }
            async fn execute(&self, _ctx: &ExecuteContext, _args: serde_json::Value) -> Result<String, String> {
                Err("se rompio".into())
            }
        }

        let concurrent = StdArc::new(AtomicUsize::new(0));
        let peak = StdArc::new(AtomicUsize::new(0));
        let pending: Vec<PreparedToolCall> = vec![
            prepared("boom", Arc::new(Failing), "x"),
            prepared("t", SlowTool::new("ok", 60, concurrent.clone(), peak.clone()), "y"),
            // Una entrada ya resuelta en la fase 1 (denegada o tool
            // inexistente): no se ejecuta y aun asi ocupa su hueco en el
            // vector de salida, para no desalinear los indices.
            pre_resolved("z", "[denied by user]"),
        ];

        let (out, _elapsed) = exec_batch_parallel(&ctx(), &pending).await;

        assert_eq!(out.len(), 3, "la salida tiene un hueco por cada entrada");
        assert!(out[0].as_ref().expect("hay resultado").is_err(), "la que falla falla");
        assert_eq!(out[1].as_ref().expect("hay resultado").as_deref(), Ok("y"));
        assert!(out[2].is_none(), "la entrada cerrada no produce resultado");
    }

    /// EP-2026-10-03: una tool ya resuelta en la fase 1 (denegada por el
    /// usuario, o inexistente) NO se ejecuta, pero conserva su hueco en la
    /// salida. Si perdiera el hueco, los indices de la salida dejarian de
    /// corresponder con los del lote y cada resultado se atribuiria a la
    /// tool equivocada.
    #[tokio::test]
    async fn una_tool_ya_resuelta_no_roba_el_hueco_de_otra() {
        let concurrent = StdArc::new(AtomicUsize::new(0));
        let peak = StdArc::new(AtomicUsize::new(0));
        let pending: Vec<PreparedToolCall> = vec![
            pre_resolved("a", "[denied by user]"),
            prepared("t", SlowTool::new("ok", 40, concurrent.clone(), peak.clone()), "b"),
        ];

        let (out, _elapsed) = exec_batch_parallel(&ctx(), &pending).await;

        assert_eq!(out.len(), 2, "un hueco por cada entrada del lote");
        assert!(out[0].is_none(), "la denegada no se ejecuta");
        assert_eq!(out[1].as_ref().expect("resultado").as_deref(), Ok("b"));
    }

    /// Un lote vacio o de un solo item no debe hacer nada raro.
    #[tokio::test]
    async fn lote_vacio_y_lote_de_uno() {
        let (out, _e) = exec_batch_parallel(&ctx(), &[]).await;
        assert!(out.is_empty());

        let concurrent = StdArc::new(AtomicUsize::new(0));
        let peak = StdArc::new(AtomicUsize::new(0));
        let one = vec![prepared("t", SlowTool::new("t", 10, concurrent, peak), "solo")];
        let (out, _e) = exec_batch_parallel(&ctx(), &one).await;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].as_ref().expect("resultado").as_deref(), Ok("solo"));
    }
}
