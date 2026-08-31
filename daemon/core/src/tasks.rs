//! Per-session cancellation tokens.
//!
//! Each in-flight request gets its OWN token, not a shared one. When
//! a request finishes (Ok or Err), its token is removed via the
//! `token_id` returned from `create`.
//!
//! `cancel(session_id)` cancels ALL active tokens for a session — needed
//! for the "stop" button to interrupt every concurrent request.
//!
//! EP-0007: the previous `get_or_create` returned the SAME token for
//! all requests on a session, so a cancelled token would cancel
//! subsequent requests even before they started. That broke chat.

use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct TaskManager {
    /// session_id -> (token_id -> token). Each request has a unique
    /// token_id so cleanup can target the right one.
    tokens: Mutex<HashMap<Uuid, HashMap<Uuid, CancellationToken>>>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Create a NEW cancellation token for this request. Returns the
    /// token AND a `token_id` for use with `cleanup`.
    pub async fn create(&self, session_id: Uuid) -> (Uuid, CancellationToken) {
        let token = CancellationToken::new();
        let token_id = Uuid::new_v4();
        let mut g = self.tokens.lock().await;
        g.entry(session_id).or_default().insert(token_id, token.clone());
        (token_id, token)
    }

    /// Trigger cancellation for ALL active requests on this session.
    /// Returns the number of tokens cancelled.
    pub async fn cancel(&self, session_id: Uuid) -> usize {
        let g = self.tokens.lock().await;
        if let Some(map) = g.get(&session_id) {
            let count = map.len();
            for (_, token) in map {
                token.cancel();
            }
            count
        } else {
            0
        }
    }

    /// Remove a specific token from the manager. Should be called when
    /// the request that owns it completes (Ok or Err).
    pub async fn cleanup(&self, session_id: Uuid, token_id: Uuid) {
        let mut g = self.tokens.lock().await;
        if let Some(map) = g.get_mut(&session_id) {
            map.remove(&token_id);
            if map.is_empty() {
                g.remove(&session_id);
            }
        }
    }

    /// Cancel and remove all tokens. Useful on daemon shutdown.
    pub async fn cancel_all(&self) {
        let mut g = self.tokens.lock().await;
        for (_, map) in g.drain() {
            for (_, token) in map {
                token.cancel();
            }
        }
    }
}

pub type SharedTaskManager = std::sync::Arc<TaskManager>;