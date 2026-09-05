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

use crate::router::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListSessionsQuery {
    /// Optional client_id filter. When provided, the response only
    /// includes sessions whose `client_id` matches. Sessions with
    /// `client_id == NULL` are never returned in that mode.
    #[serde(default)]
    pub client_id: Option<String>,
}

/// GET /v1/sessions — list recent sessions.
///
/// With `?client_id=web` (or `?client_id=sidebar-1`), the response is
/// partitioned to that client only. With no query param, all sessions
/// are returned (legacy behavior; admins / debug).
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListSessionsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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
            .list_sessions(50)
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
            })
        })
        .collect();

    Ok(Json(json!({ "sessions": items })))
}

/// GET /v1/sessions/:id/messages — load all messages for a session.
pub async fn get_session_messages(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // EP-0030 strict-404: verify session exists before returning its
    // messages. Without this check, GET messages on a non-existent
    // session returned `{"messages": []}` with status 200, hiding the
    // error from clients.
    let sessions = state
        .lifecycle
        .session
        .list_sessions(1000)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !sessions.iter().any(|s| s.session_id == session_id.to_string()) {
        return Err((StatusCode::NOT_FOUND, "session not found".to_string()));
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
            json!({
                "id": m.id,
                "session_id": m.session_id,
                "role": m.role,
                "content": m.content,
                "thinking": m.thinking,
                "ts": m.ts,
            })
        })
        .collect();

    Ok(Json(json!({ "messages": items })))
}
