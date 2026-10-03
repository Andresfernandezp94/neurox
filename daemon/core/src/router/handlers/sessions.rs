//! EP-0013 A-001 (continued): sessions handlers extracted from
//! router/http.rs.
//!
//! Phase 1: `list_sessions` + `get_session_messages`.
//! Phase 2 (deferred): `create_session`, `post_message`, `post_message_stream`,
//! `cancel_session`, `rename_session` — those have deeper dependencies
//! (post_message uses `lifecycle.tasks`, SSE forwarders, etc.).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::UserContext;
use crate::router::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListSessionsQuery {
    /// Optional client_id filter. **Deprecated** for the main UI:
    /// sessions are now shared per-user (see `?include_inactive`). Kept
    /// for the diagnostics view so admins can still see a single
    /// device's sessions.
    #[serde(default)]
    pub client_id: Option<String>,
    /// `true` → also return closed sessions (`ended_at IS NOT NULL`).
    /// Default `false` → only active sessions (`ended_at IS NULL`) —
    /// these are the ones that show as tabs.
    #[serde(default)]
    pub include_inactive: bool,
}

/// GET /v1/sessions — list recent sessions owned by the authenticated
/// user. Active by default (closed sessions show up in a separate
/// history view, not as tabs). Pass `?include_inactive=true` to see
/// closed ones too.
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Query(q): Query<ListSessionsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Per-user filter is the only supported path for the main UI.
    // The deprecated client_id path is preserved for back-compat with
    // older clients (sidebar sessions list, debug views).
    let sessions = if let Some(cid) = q.client_id.as_deref() {
        state
            .lifecycle
            .session
            .list_sessions_by_client(cid, 50)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        state
            .lifecycle
            .session
            .list_sessions_by_user(&user.user_id.to_string(), q.include_inactive, 50)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let items: Vec<serde_json::Value> = sessions
        .into_iter()
        .map(|s| {
            json!({
                "session_id": s.session_id,
                "agent_id": s.agent_id,
                "started_at": s.started_at,
                "ended_at": s.ended_at,
                "summary": s.summary,
                "provider_id": s.provider_id,
                "model": s.model,
                "client_id": s.client_id,
                "user_id": s.user_id,
            })
        })
        .collect();

    Ok(Json(json!({ "sessions": items })))
}

/// GET /v1/sessions/:id/messages — load all messages for a session.
/// Owner-only: 404 if the session doesn't exist OR belongs to a
/// different user (the two cases are indistinguishable from the
/// outside so we don't leak existence).
pub async fn get_session_messages(
    State(state): State<Arc<AppState>>,
    user: UserContext,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Owner check: refuse to serve messages for sessions the caller
    // doesn't own. Done as a single lookup so we don't pull the full
    // session list (cheap, hits the user_id index).
    let owner = state
        .lifecycle
        .session
        .get_session_user(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match owner {
        Some(uid) if uid == user.user_id.to_string() => {}
        _ => return Err((StatusCode::NOT_FOUND, "session not found".to_string())),
    }

    let messages = state
        .lifecycle
        .session
        .get_messages(session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|m| {
            // EP-0026-rev-fix: surface the persisted thinking block in
            // the API response so the frontend can hydrate the
            // ThinkingNode on reload without needing to re-parse
            // `content`.
            //
            // EP-2026-10-03: `tool_name` y `tool_call_id` tambien tienen
            // que salir, o recargar la sesion pierde el nombre de cada
            // tool (el cliente lo pintaba como "unknown") y el `call_id`
            // que empareja la peticion con su resultado. Los dos campos
            // ya se leian de la base de datos; aqui se perdian al armar el
            // JSON a mano. Se anteponen al final para no romper a ningun
            // cliente que los ignorase.
            json!({
                "id": m.id,
                "session_id": m.session_id,
                "role": m.role,
                "content": m.content,
                "thinking": m.thinking,
                "ts": m.ts,
                "tool_name": m.tool_name,
                "tool_call_id": m.tool_call_id,
            })
        })
        .collect();

    Ok(Json(json!({ "messages": items })))
}
