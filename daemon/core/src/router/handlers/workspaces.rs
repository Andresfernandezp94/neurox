//! EP-0026-UX: handlers de `/v1/workspaces`.
//!
//! Workspaces = entornos aislados. Cada uno tiene su directorio raiz y su
//! propio sandbox, con los paths guardados sin resolver para que
//! `${workspace}` se expanda contra SU raiz y no contra la global.
//!
//! Todos exigen rol Admin: crear o borrar un entorno cambia que archivos
//! puede tocar el agente, asi que no es una operacion de lectura. Es el
//! mismo criterio que `invoke_tool`.
//!
//! `DELETE` borra el registro, NO el directorio. El directorio es del
//! usuario y puede tener trabajo adentro; remover un workspace revoca el
//! acceso, no destruye nada.

use axum::{
    extract::{Path as AxumPath, Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

use crate::auth::Role;
use crate::auth::UserContext;
use crate::config::SandboxConfig;
use crate::router::state::AppState;
use crate::workspaces::{WorkspacePatch, WorkspaceRecord};

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(serde_json::json!({ "ok": false, "error": msg.into() }))).into_response()
}

fn ok(value: serde_json::Value) -> Response {
    (StatusCode::OK, Json(value)).into_response()
}

/// Admin para todo lo de workspaces. Devuelve la respuesta de rechazo o
/// `None` para seguir.
fn require_admin(ctx: &UserContext) -> Option<Response> {
    if ctx.role != Role::Admin {
        return Some(err(
            StatusCode::FORBIDDEN,
            "admin role required to manage workspaces",
        ));
    }
    None
}

/// `AppState` tiene las capas de workspaces en `Option`: si no se
/// cablearon (tests, o un daemon sin la tabla) la API responde 503 en vez
/// de fingir que hay workspaces.
macro_rules! ws_layer {
    ($state:expr) => {
        match $state.workspaces.clone() {
            Some(l) => l,
            None => {
                return err(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "workspaces no esta habilitado en este daemon",
                )
            }
        }
    };
}

/// Valida una lista de paths de sandbox.
///
/// Mismo criterio que `PUT /v1/sandbox`: sin entradas vacias (un path
/// vacio matchea todo en `resolve_under_workspace`) y sin `..` (traversal).
fn validate_paths(kind: &str, paths: &[String]) -> Result<(), String> {
    for p in paths {
        if p.trim().is_empty() {
            return Err(format!("{kind} entries must be non-empty"));
        }
        if p.contains("..") {
            return Err(format!("{kind} entry '{p}' contains '..' (traversal not allowed)"));
        }
    }
    Ok(())
}

fn validate_root(root: &str) -> Result<(), String> {
    if root.trim().is_empty() {
        return Err("root must be non-empty".into());
    }
    if !Path::new(root).is_absolute() {
        // Relativo no aisla nada: cada tool lo resolveria contra su propio
        // cwd y dos workspaces con nombres parecidos podrian caer en
        // directorios distintos sin que nadie lo note.
        return Err(format!("root must be an absolute path, got '{root}'"));
    }
    Ok(())
}

fn validate_patch(patch: &WorkspacePatch) -> Result<(), String> {
    if let Some(paths) = &patch.sandbox_writable_paths {
        validate_paths("writable_paths", paths)?;
    }
    if let Some(paths) = &patch.sandbox_readable_paths {
        validate_paths("readable_paths", paths)?;
    }
    if let Some(d) = patch.sandbox_max_recursion_depth {
        if d == 0 {
            return Err("sandbox_max_recursion_depth must be > 0".into());
        }
    }
    if let Some(name) = &patch.name {
        if name.trim().is_empty() {
            return Err("name must be non-empty".into());
        }
    }
    if let Some(root) = &patch.root {
        validate_root(root)?;
    }
    Ok(())
}

fn record_json(w: &WorkspaceRecord) -> serde_json::Value {
    serde_json::json!({
        "id": w.id,
        "name": w.name,
        "description": w.description,
        "root": w.root,
        "status": w.status.as_str(),
        "icon": w.icon,
        "is_default": w.is_default,
        "sandbox": {
            "enabled": w.sandbox.enabled,
            "writable_paths": w.sandbox.writable_paths,
            "readable_paths": w.sandbox.readable_paths,
            "max_recursion_depth": w.sandbox.max_recursion_depth,
        },
        "created_at": w.created_at,
        "updated_at": w.updated_at,
    })
}

// ─── GET /v1/workspaces ─────────────────────────────────────────────────────

/// Lista los workspaces configurados, el default primero.
pub async fn list_workspaces(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
) -> Response {
    if let Some(r) = require_admin(&ctx) {
        return r;
    }
    let l = ws_layer!(state);
    match l.store.list().await {
        Ok(list) => ok(serde_json::json!({
            "ok": true,
            "workspaces": list.iter().map(record_json).collect::<Vec<_>>()
        })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("list workspaces: {e}")),
    }
}

// ─── POST /v1/workspaces ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateReq {
    pub name: String,
    /// Directorio raiz del workspace. Tiene que ser absoluto.
    pub root: String,
    #[serde(flatten)]
    pub patch: WorkspacePatch,
}

/// Crea un workspace.
///
/// No crea el directorio: es una escritura en el filesystem y el operador
/// puede querer apuntar a uno que todavia no creo a proposito.
pub async fn create_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
    Json(body): Json<CreateReq>,
) -> Response {
    if let Some(r) = require_admin(&ctx) {
        return r;
    }
    let mut patch = body.patch;
    patch.name.get_or_insert(body.name.clone());
    if let Err(e) = validate_patch(&patch).and_then(|_| validate_root(&body.root)) {
        return err(StatusCode::BAD_REQUEST, e);
    }

    let l = ws_layer!(state);
    let created = match l.store.create(&body.name, &body.root, &patch).await {
        Ok(w) => w,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, format!("create workspace: {e}")),
    };
    // El registro tiene que saber el sandbox del workspace nuevo, o su
    // primera llamada caeria al default global con permisos distintos de
    // los que el operador acaba de configurar.
    l.registry
        .set_sandbox(&created.id, created.sandbox_config())
        .await;

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "ok": true, "workspace": record_json(&created) })),
    )
        .into_response()
}

// ─── GET /v1/workspaces/:id ─────────────────────────────────────────────────

pub async fn get_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(r) = require_admin(&ctx) {
        return r;
    }
    let l = ws_layer!(state);
    match l.store.get(&id).await {
        Ok(Some(w)) => ok(serde_json::json!({ "ok": true, "workspace": record_json(&w) })),
        Ok(None) => err(StatusCode::NOT_FOUND, format!("workspace not found: {id}")),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("get workspace: {e}")),
    }
}

// ─── PATCH /v1/workspaces/:id ───────────────────────────────────────────────

/// Actualiza un workspace. Parcial: lo que no viene en el body no se toca.
pub async fn update_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
    AxumPath(id): AxumPath<String>,
    Json(patch): Json<WorkspacePatch>,
) -> Response {
    if let Some(r) = require_admin(&ctx) {
        return r;
    }
    if let Err(e) = validate_patch(&patch) {
        return err(StatusCode::BAD_REQUEST, e);
    }
    let l = ws_layer!(state);
    let updated = match l.store.update(&id, &patch).await {
        Ok(w) => w,
        Err(e) if e.to_string().contains("not found") => {
            return err(StatusCode::NOT_FOUND, format!("workspace not found: {id}"))
        }
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, format!("update workspace: {e}")),
    };
    // Si el patch toco el sandbox, la celda viva tiene que cambiar con el.
    if patch.sandbox_enabled.is_some()
        || patch.sandbox_writable_paths.is_some()
        || patch.sandbox_readable_paths.is_some()
        || patch.sandbox_max_recursion_depth.is_some()
    {
        l.registry
            .set_sandbox(&updated.id, updated.sandbox_config())
            .await;
    }

    ok(serde_json::json!({ "ok": true, "workspace": record_json(&updated) }))
}

// ─── DELETE /v1/workspaces/:id ──────────────────────────────────────────────

/// Remueve el workspace. No borra el directorio.
pub async fn delete_workspace(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(r) = require_admin(&ctx) {
        return r;
    }
    let l = ws_layer!(state);
    match l.store.delete(&id).await {
        Ok(()) => {
            // Relaja la celda: si un scope en vuelo ya la cloneo, tiene que
            // dejar de autorizar con los permisos de un workspace que el
            // operador borro.
            l.registry.remove(&id).await;
            ok(serde_json::json!({ "ok": true, "deleted": id }))
        }
        Err(e) if e.to_string().contains("not found") => {
            err(StatusCode::NOT_FOUND, format!("workspace not found: {id}"))
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("delete workspace: {e}")),
    }
}

// ─── PUT /v1/workspaces/:id/sandbox ─────────────────────────────────────────

/// Parchea el sandbox de UN workspace.
///
/// Contraparte de `PUT /v1/sandbox`, que mueve el global. Los dos
/// conviven: el global sigue siendo el default de las sesiones sin
/// workspace.
pub async fn put_workspace_sandbox(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
    AxumPath(id): AxumPath<String>,
    Json(patch): Json<WorkspacePatch>,
) -> Response {
    if let Some(r) = require_admin(&ctx) {
        return r;
    }
    if let Err(e) = validate_patch(&patch) {
        return err(StatusCode::BAD_REQUEST, e);
    }
    let l = ws_layer!(state);
    if l.store.get(&id).await.ok().flatten().is_none() {
        return err(StatusCode::NOT_FOUND, format!("workspace not found: {id}"));
    }
    let solo_sandbox = WorkspacePatch {
        sandbox_enabled: patch.sandbox_enabled,
        sandbox_writable_paths: patch.sandbox_writable_paths,
        sandbox_readable_paths: patch.sandbox_readable_paths,
        sandbox_max_recursion_depth: patch.sandbox_max_recursion_depth,
        ..Default::default()
    };
    let updated = match l.store.update(&id, &solo_sandbox).await {
        Ok(w) => w,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, format!("update sandbox: {e}")),
    };
    l.registry
        .set_sandbox(&updated.id, updated.sandbox_config())
        .await;

    ok(serde_json::json!({ "ok": true, "applied": true, "workspace": record_json(&updated) }))
}

// ─── GET /v1/workspaces/sandbox-defaults ────────────────────────────────────

/// Defaults con los que se crea un workspace, para que el form del front no
/// los hardcodee y se desincronice del backend en cuanto cambien.
pub async fn get_sandbox_defaults(State(state): State<Arc<AppState>>) -> Response {
    let _ = &state;
    let d = SandboxConfig::default();
    ok(serde_json::json!({
        "ok": true,
        "defaults": {
            "enabled": d.enabled,
            "writable_paths": d.writable_paths,
            "readable_paths": d.readable_paths,
            "max_recursion_depth": d.max_recursion_depth,
        },
        "statuses": ["active", "paused", "draft"],
    }))
}

/// Variante de `list_workspaces` sin el chequeo de rol, para el panel de
/// chat que solo necesita conocer el default. No expone paths.
pub async fn list_workspace_names(State(state): State<Arc<AppState>>) -> Response {
    let Some(l) = state.workspaces.clone() else {
        return ok(serde_json::json!({ "ok": true, "workspaces": [] }));
    };
    match l.store.list().await {
        Ok(list) => ok(serde_json::json!({
            "ok": true,
            "workspaces": list
                .iter()
                .map(|w| serde_json::json!({
                    "id": w.id,
                    "name": w.name,
                    "is_default": w.is_default,
                    "status": w.status.as_str(),
                }))
                .collect::<Vec<_>>()
        })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("list workspaces: {e}")),
    }
}

#[cfg(test)]
#[path = "tests_workspaces.rs"]
mod tests;
