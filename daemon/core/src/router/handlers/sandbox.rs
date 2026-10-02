//! EP-0013 A-001 (continued): sandbox handlers extracted from
//! router/http.rs.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use std::path::PathBuf;

use crate::router::state::AppState;

/// PUT /v1/sandbox — hot-patch the sandbox config (EP-0019-03).
///
/// Body (all fields optional):
/// ```json
/// {
///   "enabled": true,
///   "writable_paths": ["${workspace}/.sdd", "/tmp/foo"],
///   "max_recursion_depth": 5
/// }
/// ```
///
/// Returns the new full sandbox config. Applies changes to all tools
/// (they read from the `Arc<RwLock<Box<dyn SandboxConfig>>>` on every call)
/// and re-renders the in-process default's system prompt template.
pub async fn put_sandbox(
    State(state): State<Arc<AppState>>,
    Json(patch): Json<SandboxPatch>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // EP-0015: PUT /v1/sandbox now has real effect. We construct a new
    // `core::config::SandboxConfig` from the patch, then atomically swap
    // it into the engine's `Arc<RwLock<Box<dyn SandboxConfig>>>` via
    // `engine.set_sandbox()`.

    // Validate payloads
    if let Some(paths) = &patch.writable_paths {
        for p in paths {
            if p.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "writable_paths entries must be non-empty".into(),
                ));
            }
            if p.contains("..") {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "writable_paths entry '{}' contains '..' (traversal not allowed)",
                        p
                    ),
                ));
            }
        }
    }
    if let Some(depth) = patch.max_recursion_depth {
        if depth == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "max_recursion_depth must be > 0".into(),
            ));
        }
    }

    // Build the new config from the patch
    let mut new_cfg = crate::config::SandboxConfig::default();
    if let Some(enabled) = patch.enabled {
        new_cfg.enabled = enabled;
    }
    if let Some(paths) = patch.writable_paths {
        new_cfg.writable_paths = paths;
    }
    if let Some(paths) = patch.readable_paths {
        new_cfg.readable_paths = paths;
    }
    if let Some(depth) = patch.max_recursion_depth {
        new_cfg.max_recursion_depth = depth;
    }

    // Un solo Arc: el del engine (compartido con todas las tools) y el del
    // `WorkspaceLayer` son el mismo objeto. Por eso alcanza con escribir una
    // vez, via `set_sandbox`, que ademas es la via que usan las tools.
    state.engine.set_sandbox(Box::new(new_cfg.clone())).await;

    tracing::info!(
        new_enabled = new_cfg.enabled,
        new_paths = ?new_cfg.writable_paths,
        new_depth = new_cfg.max_recursion_depth,
        "sandbox patched (engine + workspace)"
    );

    let mut response = serde_json::to_value(&new_cfg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize sandbox: {e}")))?;
    if let Some(obj) = response.as_object_mut() {
        obj.insert("applied".to_string(), serde_json::Value::Bool(true));
    }
    Ok(Json(response))
}

/// GET /v1/sandbox — read the current sandbox config without mutating it.
pub async fn get_sandbox(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let s = state.workspace.sandbox.read().await;
    let json = serde_json::json!({
        "enabled": s.enabled(),
        "writable_paths": s.writable_paths_resolved(&PathBuf::new()),
        "readable_paths": s.readable_paths_resolved(&PathBuf::new()),
        "max_recursion_depth": s.max_recursion_depth().unwrap_or(10),
    });
    Json(json)
}

/// Body of `PUT /v1/sandbox`. All fields optional.
#[derive(Debug, Deserialize)]
pub struct SandboxPatch {
    pub enabled: Option<bool>,
    pub writable_paths: Option<Vec<String>>,
    /// EP-0024: paths where read tools are allowed but writes are rejected.
    pub readable_paths: Option<Vec<String>>,
    pub max_recursion_depth: Option<usize>,
}
