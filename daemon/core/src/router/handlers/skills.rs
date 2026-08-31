//! EP-0013 A-001: skills handlers (extracted from router/http.rs).
//!
//! Each handler is independent and only uses shared `AppState` types
//! (no internal state between handlers). This is the simplest extraction
//! possible — a template for partitioning sessions/agents/tools later.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::router::state::AppState;

pub async fn enable_skill(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.lifecycle.skills.enable(&name);
    tracing::info!(skill = %name, "skill enabled");
    Ok(Json(json!({
        "ok": true,
        "name": name,
        "enabled": true,
    })))
}

pub async fn disable_skill(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.lifecycle.skills.disable(&name);
    tracing::info!(skill = %name, "skill disabled");
    Ok(Json(json!({
        "ok": true,
        "name": name,
        "enabled": false,
    })))
}
