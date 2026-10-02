//! Tests de los handlers de `/v1/workspaces`.
//!
//! Se llaman a los handlers directo en vez de por HTTP porque el
//! `Extension<UserContext>` lo inyecta el `JwtAuthLayer`: para probarlo
//! hace falta un `AppState` con la capa de workspaces, y `oneshot` sobre el
//! router agregaria el JWT y el puerto, sin ganar nada.
//!
//! Lo que importa acá es el efecto, no el status: que crear un workspace
//! le de SU sandbox y no el global, que editarlo no mueva al otro, y que
//! borrar uno no deje autorizando con los permisos viejos.

use super::*;
use crate::router::state::{AppState, AuthLayer, EventsLayer, WorkspaceLayer};
use crate::workspaces::WorkspacesLayer;
use axum::extract::Extension;
use std::path::PathBuf;

fn admin() -> UserContext {
    UserContext {
        user_id: uuid::Uuid::new_v4(),
        username: "tester".into(),
        role: Role::Admin,
    }
}

fn viewer() -> UserContext {
    UserContext {
        user_id: uuid::Uuid::new_v4(),
        username: "viewer".into(),
        role: Role::Viewer,
    }
}

/// `workspaces_enabled = false` deja el CRUD vivo pero la resolucion de
/// scope ignorando el workspace. Ver `state_apagados`.
async fn state() -> (AppState, tempfile::TempDir) {
    build_state(true).await
}

async fn build_state(workspaces_enabled: bool) -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("w.db");

    let tools = Arc::new(tools_engine::tools::ToolRegistry::new());
    let sandbox: Arc<tokio::sync::RwLock<Box<dyn tools_engine::SandboxConfig>>> = Arc::new(tokio::sync::RwLock::new(
        Box::new(SandboxConfig {
            readable_paths: vec!["/home".into()],
            ..Default::default()
        }) as Box<dyn tools_engine::SandboxConfig>,
    ));
    tools_engine::tools::register_defaults(&tools, PathBuf::from("/tmp"), sandbox.clone());
    let engine = tools_engine::Engine::for_testing(tools, PathBuf::from("/tmp"), sandbox.clone())
        .await
        .unwrap();

    let workspace = Arc::new(WorkspaceLayer::new(PathBuf::from("/tmp"), sandbox.clone()));
    let layers = WorkspacesLayer::open(&db, sandbox).await.unwrap();

    let state = AppState::new(
        Arc::new(crate::router::state::LifecycleLayer::new(
            Arc::new(crate::registry::Registry::new(PathBuf::from("/tmp"))),
            Arc::new(crate::supervisor::Supervisor::new()),
            Arc::new(crate::spawner::Spawner::new(4)),
            Arc::new(crate::tasks::TaskManager::new()),
            Arc::new(crate::approval::ApprovalManager::default()),
            Arc::new(crate::session_agents::SessionAgentPool::default()),
            Arc::new(crate::session::SessionStore::open(&db).await.unwrap()),
            Arc::new(crate::skills::SkillsRegistry::new()),
            Arc::new(crate::plugins::PluginToolRegistry::new(
                Arc::new(tools_engine::tools::ToolRegistry::new()),
            )),
        )),
        Arc::new(EventsLayer::new(tokio::sync::broadcast::channel(16).0)),
        engine,
        AuthLayer::new(),
        workspace,
        Arc::new(crate::config::CoreConfig {
            workspaces: crate::config::WorkspacesSection { enabled: workspaces_enabled },
            ..Default::default()
        }),
    )
    .with_workspaces(Arc::new(layers));

    (state, tmp)
}

fn body_of(res: Response) -> serde_json::Value {
    let bytes = futures::executor::block_on(
        axum::body::to_bytes(res.into_body(), 1 << 20),
    )
    .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

async fn create(
    state: &AppState,
    name: &str,
    root: &str,
    readable: &[&str],
) -> serde_json::Value {
    let req = CreateReq {
        name: name.into(),
        root: root.into(),
        patch: WorkspacePatch {
            sandbox_readable_paths: Some(readable.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        },
    };
    let res = create_workspace(State(Arc::new(state.clone())), Extension(admin()), Json(req)).await;
    let status = res.status();
    let b = body_of(res);
    assert_eq!(status, StatusCode::CREATED, "create {name}: {b:?}");
    b["workspace"].clone()
}

#[tokio::test]
async fn listar_devuelve_empty_sin_workspaces() {
    let (state, _t) = state().await;
    let res = list_workspaces(State(Arc::new(state.clone())), Extension(admin())).await;
    assert_eq!(res.status(), StatusCode::OK);
    let b = body_of(res);
    assert_eq!(b["workspaces"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn crear_listar_y_borrar() {
    let (state, _t) = state().await;
    create(&state, "Alfa", "/srv/alfa", &["${workspace}"]).await;

    let res = list_workspaces(State(Arc::new(state.clone())), Extension(admin())).await;
    let b = body_of(res);
    assert_eq!(b["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(b["workspaces"][0]["id"], "alfa");
    assert_eq!(b["workspaces"][0]["root"], "/srv/alfa");

    let res = delete_workspace(State(Arc::new(state.clone())), Extension(admin()), AxumPath("alfa".into())).await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = list_workspaces(State(Arc::new(state.clone())), Extension(admin())).await;
    assert_eq!(body_of(res)["workspaces"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn sin_rol_de_admin_no_se_puede_tocar_nada() {
    let (state, _t) = state().await;
    create(&state, "Alfa", "/srv/alfa", &[]).await;

    let req = CreateReq {
        name: "Otro".into(),
        root: "/srv/otro".into(),
        patch: WorkspacePatch::default(),
    };
    let res = create_workspace(State(Arc::new(state.clone())), Extension(viewer()), Json(req)).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = list_workspaces(State(Arc::new(state.clone())), Extension(viewer())).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = delete_workspace(State(Arc::new(state.clone())), Extension(viewer()), AxumPath("alfa".into())).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Y el workspace sigue ahi: un rechazo no deja efectos secundarios.
    let res = list_workspaces(State(Arc::new(state.clone())), Extension(admin())).await;
    assert_eq!(body_of(res)["workspaces"].as_array().unwrap().len(), 1);
}

/// Root relativo no aisla nada: cada tool lo resolveria contra su propio
/// cwd. Se rechaza en create y en patch.
#[tokio::test]
async fn root_relativo_se_rechaza() {
    let (state, _t) = state().await;
    let req = CreateReq {
        name: "Relativo".into(),
        root: "proyectos/mio".into(),
        patch: WorkspacePatch::default(),
    };
    let res = create_workspace(State(Arc::new(state.clone())), Extension(admin()), Json(req)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_of(res)["error"].as_str().unwrap().contains("absolute"));

    let req = CreateReq { name: "Ok".into(), root: "/srv/ok".into(), patch: WorkspacePatch::default() };
    let _ = create_workspace(State(Arc::new(state.clone())), Extension(admin()), Json(req)).await;
    let res = update_workspace(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("ok".into()),
        Json(WorkspacePatch { root: Some("relativo".into()), ..Default::default() }),
    ).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

/// Un path vacio matchea todo en `resolve_under_workspace`, y un `..` es
/// traversal. Los dos se rechazan.
#[tokio::test]
async fn paths_de_sandbox_se_validan() {
    let (state, _t) = state().await;
    let req = CreateReq { name: "Ok".into(), root: "/srv/ok".into(), patch: WorkspacePatch::default() };
    let _ = create_workspace(State(Arc::new(state.clone())), Extension(admin()), Json(req)).await;

    let res = put_workspace_sandbox(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("ok".into()),
        Json(WorkspacePatch { sandbox_readable_paths: Some(vec!["  ".into()]), ..Default::default() }),
    ).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "path vacio");

    let res = put_workspace_sandbox(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("ok".into()),
        Json(WorkspacePatch { sandbox_readable_paths: Some(vec!["/a/../../etc".into()]), ..Default::default() }),
    ).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "traversal");

    let res = put_workspace_sandbox(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("ok".into()),
        Json(WorkspacePatch { sandbox_max_recursion_depth: Some(0), ..Default::default() }),
    ).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "depth 0");
}

/// El motivo de ser del feature: el sandbox del workspace con el que se
/// creo, no el global. Y el `${workspace}` se expande contra SU root.
#[tokio::test]
async fn el_workspace_nace_con_su_sandbox_y_no_con_el_global() {
    let (state, _t) = state().await;
    create(&state, "Alfa", "/srv/alfa", &["${workspace}"]).await;

    let l = state.workspaces.clone().unwrap();
    let celda = l.registry.sandbox_for("alfa").await;
    let leidos = celda.read().await.readable_paths_resolved(Path::new("/srv/alfa"));
    assert_eq!(leidos, vec![PathBuf::from("/srv/alfa")], "el placeholder de workspace apunta a su propio root");
    assert_ne!(
        leidos,
        vec![PathBuf::from("/home")],
        "no puede haber heredado el legible del sandbox global"
    );
}

/// El punto critico: editar el sandbox de A no cambia lo que B puede
/// leer. Con un Arc compartido esto seria imposible.
#[tokio::test]
async fn editar_el_sandbox_de_uno_no_mueve_el_otro() {
    let (state, _t) = state().await;
    create(&state, "A", "/srv/a", &["/srv/a"]).await;
    create(&state, "B", "/srv/b", &["/srv/b"]).await;

    let res = put_workspace_sandbox(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("a".into()),
        Json(WorkspacePatch {
            sandbox_readable_paths: Some(vec!["/srv/a2".into()]),
            ..Default::default()
        }),
    ).await;
    assert_eq!(res.status(), StatusCode::OK);

    let l = state.workspaces.clone().unwrap();
    let ra = l.registry.sandbox_for("a").await.read().await.readable_paths_resolved(Path::new("/srv/a"));
    let rb = l.registry.sandbox_for("b").await.read().await.readable_paths_resolved(Path::new("/srv/b"));
    assert_eq!(ra, vec![PathBuf::from("/srv/a2")], "A cambio");
    assert_eq!(rb, vec![PathBuf::from("/srv/b")], "B intacto");
}

/// Borrar relaja la celda viva. Sin esto, un scope ya armado seguiria
/// autorizando con los permisos de un workspace que el operador borro.
#[tokio::test]
async fn borrar_relaja_la_celda_que_alguien_ya_tomo() {
    let (state, _t) = state().await;
    create(&state, "A", "/srv/a", &["/srv/a"]).await;

    let l = state.workspaces.clone().unwrap();
    let celda_tomada = l.registry.sandbox_for("a").await;

    let res = delete_workspace(State(Arc::new(state.clone())), Extension(admin()), AxumPath("a".into())).await;
    assert_eq!(res.status(), StatusCode::OK);

    assert!(
        celda_tomada.read().await.readable_paths_resolved(Path::new("/srv/a")).is_empty(),
        "la celda en vuelo tiene que dejar de autorizar"
    );
}

/// Un 404 en vez de un error 500 cuando el id no existe. Un handler que
/// devuelve 500 por un id mal escrito esconde el problema real.
#[tokio::test]
async fn id_inexistente_da_404() {
    let (state, _t) = state().await;
    let res = get_workspace(State(Arc::new(state.clone())), Extension(admin()), AxumPath("nope".into())).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = delete_workspace(State(Arc::new(state.clone())), Extension(admin()), AxumPath("nope".into())).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = update_workspace(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("nope".into()),
        Json(WorkspacePatch { name: Some("x".into()), ..Default::default() }),
    ).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = put_workspace_sandbox(
        State(Arc::new(state.clone())),
        Extension(admin()),
        AxumPath("nope".into()),
        Json(WorkspacePatch { sandbox_enabled: Some(false), ..Default::default() }),
    ).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Los defaults que anuncia el endpoint tienen que ser los que de verdad
/// usa `create`: si divergen, el form del front arma workspaces distintos
/// de los que el operador cree.
#[tokio::test]
async fn los_defaults_anunciados_son_los_reales() {
    let (state, _t) = state().await;
    let res = get_sandbox_defaults(State(Arc::new(state.clone()))).await;
    let b = body_of(res);
    let d = &b["defaults"];
    let real = SandboxConfig::default();
    assert_eq!(d["enabled"], real.enabled);
    assert_eq!(d["max_recursion_depth"], real.max_recursion_depth);
    assert_eq!(
        d["readable_paths"].as_array().unwrap().len(),
        real.readable_paths.len()
    );
}

/// El switch `workspaces.enabled: false` apaga la APLICACION, no el
/// almacen: el CRUD sigue vivo para poder preparar los entornos con el
/// feature apagado y activarlos despues.
mod master_switch {
    use super::*;

    #[tokio::test]
    async fn apagado_el_scope_de_la_sesion_cae_al_global() {
        let (state, _t) = build_state(false).await;
        create(&state, "Alfa", "/srv/alfa", &["${workspace}"]).await;

        let sid = uuid::Uuid::new_v4();
        state
            .lifecycle
            .session
            .start_session_for_user(sid, "default", &uuid::Uuid::new_v4().to_string())
            .await
            .unwrap();
        state.lifecycle.session.set_workspace_id(sid, Some("alfa")).await.unwrap();

        // El id esta en la sesion, pero el switch lo ignora.
        let sc = state.scope_for_session(sid).await;
        assert_eq!(sc.id, None, "con el switch apagado no se aplica el workspace");
        assert_eq!(sc.root, PathBuf::from("/tmp"), "el root es el global del test");
    }

    #[tokio::test]
    async fn apagado_el_scope_del_agente_tambien_cae_al_global() {
        let (state, _t) = build_state(false).await;
        create(&state, "Alfa", "/srv/alfa", &["${workspace}"]).await;
        let sc = state.scope_for_agent("default").await;
        assert_eq!(sc.id, None);
        assert_eq!(sc.root, PathBuf::from("/tmp"));
    }

    #[tokio::test]
    async fn apagado_el_crud_sigue_funcionando() {
        let (state, _t) = build_state(false).await;
        // Se puede seguir creando, leyendo y editando.
        create(&state, "Alfa", "/srv/alfa", &["${workspace}"]).await;
        let res = list_workspaces(State(Arc::new(state.clone())), Extension(admin())).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_of(res)["workspaces"].as_array().unwrap().len(), 1);

        let res = update_workspace(
            State(Arc::new(state.clone())),
            Extension(admin()),
            AxumPath("alfa".into()),
            Json(WorkspacePatch { name: Some("Renombrado".into()), ..Default::default() }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_of(res)["workspace"]["name"], "Renombrado");
    }

    /// Encendido, el workspace se aplica: es el otro lado del switch.
    #[tokio::test]
    async fn encendido_el_workspace_se_aplica() {
        let (state, _t) = build_state(true).await;
        create(&state, "Alfa", "/srv/alfa", &["${workspace}"]).await;
        let sid = uuid::Uuid::new_v4();
        state
            .lifecycle
            .session
            .start_session_for_user(sid, "default", &uuid::Uuid::new_v4().to_string())
            .await
            .unwrap();
        state.lifecycle.session.set_workspace_id(sid, Some("alfa")).await.unwrap();

        let sc = state.scope_for_session(sid).await;
        assert_eq!(sc.id.as_deref(), Some("alfa"));
        assert_eq!(sc.root, PathBuf::from("/srv/alfa"));
    }

    /// El default del config es activado: con la tabla vacia no hay nada
    /// que aplicar y el feature queda inerte solo.
    #[tokio::test]
    async fn el_default_del_config_esta_encendido() {
        let d = crate::config::WorkspacesSection::default();
        assert!(d.enabled, "con la tabla vacia no aplica nada; se opta por el opt-out");
    }
}
