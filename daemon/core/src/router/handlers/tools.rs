//! EP-0013 A-001 (continued): tools handlers extracted from router/http.rs.
//!
//! `invoke_tool` is the HTTP-side entry point for invoking a registered
//! tool by name (admin-only since EP-0011 S-004).

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::auth::UserContext;
use crate::router::state::AppState;

/// POST /v1/tools/:name/invoke — invoke a registered tool by name.
///
/// Approval gating: the chat-side flow (`handle_session_tool_call`)
/// blocks on `requires_approval=true` tools and prompts the user. For
/// this HTTP endpoint we deliberately skip approval — admins calling
/// tools directly are already trusted, and the chat path remains the
/// gate for the LLM.
///
/// Status codes:
///   200 — `{ok: true, result: <string>}` on success
///   404 — `{ok: false, error: "tool not found"}` if no tool matches
///   502 — `{ok: false, error: <string>}` if the tool returned Err (which
///         usually means the plugin HTTP call failed)
pub async fn invoke_tool(
    Path(name): Path<String>,
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<UserContext>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    // EP-0011 S-004: tool invocation requires Admin role.
    if ctx.role != crate::auth::Role::Admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "ok": false,
                "error": "admin role required for tool invocation",
            })),
        )
            .into_response();
    }

    let Some(tool) = state.engine.tools.get(&name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "ok": false,
                "tool": name,
                "error": format!("tool not found: {name}"),
            })),
        )
            .into_response();
    };

    let spec = tool.spec();
    let args = body
        .get("args")
        .or_else(|| body.get("arguments"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // EP-0026-UX: `workspace_id` opcional en el body. Sin el, la invocacion
    // usa el sandbox global, que es el comportamiento previo.
    //
    // Sin esto no habia forma de probar el sandbox de un workspace por API:
    // el dispatch de sesion arma el scope, pero este endpoint es
    // sessionless por diseno, asi que no podia resolver nada por su cuenta.
    let workspace_id = body.get("workspace_id").and_then(|v| v.as_str());
    let scope = match (state.workspaces.as_ref(), workspace_id) {
        (Some(layers), Some(id)) => {
            crate::workspaces::resolve_scope(
                &layers.store,
                &layers.registry,
                Some(id),
                &state.workspace.workspace_root,
            )
            .await
        }
        _ => tools_engine::WorkspaceScope {
            id: None,
            root: state.workspace.workspace_root.clone(),
            sandbox: state.workspace.sandbox.clone(),
        },
    };

    let started = std::time::Instant::now();
    let ctx = tools_engine::ExecuteContext {
        agent_id: String::new(),
        workspace: Some(scope),
        cancel: None,
        http_client: Some(state.engine.http_client.clone()),
    };
    match tool.execute(&ctx, args).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "tool": spec.name,
                "result": result,
                "duration_ms": started.elapsed().as_millis() as u64,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "ok": false,
                "tool": spec.name,
                "error": e,
                "duration_ms": started.elapsed().as_millis() as u64,
            })),
        )
            .into_response(),
    }
}
