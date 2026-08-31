//! EP-0013 A-001 (continued): tools_admin handlers extracted from
//! router/http.rs.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::router::state::AppState;

/// GET /v1/tools — list all registered tools with their metadata.
pub async fn list_tools(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let tools: Vec<serde_json::Value> = state
        .engine
        .tools
        .list_specs_detailed()
        .into_iter()
        .map(|s| {
            json!({
                "name": s.spec.name,
                "description": s.spec.description,
                "requires_approval": s.spec.requires_approval,
                "parameters": s.spec.parameters,
                "enabled": s.enabled,
                "categories": s.spec.categories,
                "mode_compatible": s.spec.mode_compatible,
            })
        })
        .collect();
    Json(json!({ "tools": tools }))
}

/// POST /v1/tools/:name/enable — clear the disabled flag for a tool.
pub async fn enable_tool(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let registered = state.engine.tools.set_enabled(&name, true);
    if !registered {
        return Err((
            StatusCode::NOT_FOUND,
            format!("tool not found: {name}"),
        ));
    }
    tracing::info!(tool = %name, "tool enabled");
    Ok(Json(json!({
        "ok": true,
        "name": name,
        "enabled": true,
    })))
}

/// POST /v1/tools/:name/disable — disable a registered tool. Disabled
/// tools are skipped by `ToolRegistry::get`, so the chat-side
/// `handle_session_tool_call` and `POST /v1/tools/:name/invoke` both
/// reject them with "tool not found".
pub async fn disable_tool(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let registered = state.engine.tools.set_enabled(&name, false);
    if !registered {
        return Err((
            StatusCode::NOT_FOUND,
            format!("tool not found: {name}"),
        ));
    }
    tracing::info!(tool = %name, "tool disabled");
    Ok(Json(json!({
        "ok": true,
        "name": name,
        "enabled": false,
    })))
}
