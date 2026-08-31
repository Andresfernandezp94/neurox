//! Approval flow for dangerous tools.
//!
//! When an agent wants to invoke a tool marked `requires_approval`,
//! the core creates an `ApprovalRequest`, broadcasts an
//! `approval_request` event, and waits for the user to respond.
//!
//! The user can respond via:
//! - HTTP POST /v1/approvals/:id/respond { "decision": "approve"|"deny" }
//! - WebSocket command { "type": "`approval_response`", "id": "...", "decision": "..." }
//!
//! Meanwhile the agent call blocks on a oneshot channel until the
//! user responds or the request times out.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub session_id: Uuid,
    pub tool: String,
    pub args: serde_json::Value,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    pub id: Uuid,
    pub session_id: Uuid,
    pub tool: String,
    pub decision: ApprovalDecision,
}

struct PendingApproval {
    request: ApprovalRequest,
    tx: oneshot::Sender<ApprovalDecision>,
    created_at: Instant,
}

pub struct ApprovalManager {
    pending: Mutex<HashMap<Uuid, PendingApproval>>,
    timeout: Duration,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

impl ApprovalManager {
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            timeout,
        }
    }

    /// Create an approval request and wait for user response.
    /// Returns the decision (Approve or Deny), or Deny on timeout.
    pub async fn request(
        &self,
        session_id: Uuid,
        tool: String,
        args: serde_json::Value,
        reason: Option<String>,
    ) -> (Uuid, ApprovalDecision) {
        let id = Uuid::new_v4();
        self.request_with_id(id, session_id, tool, args, reason)
            .await
    }

    /// Create an approval request with a specific ID and wait for response.
    pub async fn request_with_id(
        &self,
        id: Uuid,
        session_id: Uuid,
        tool: String,
        args: serde_json::Value,
        reason: Option<String>,
    ) -> (Uuid, ApprovalDecision) {
        let request = ApprovalRequest {
            id,
            session_id,
            tool,
            args,
            reason,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let (tx, rx) = oneshot::channel();
        let pending = PendingApproval {
            request,
            tx,
            created_at: Instant::now(),
        };
        self.pending.lock().await.insert(id, pending);

        // Wait for response with timeout
        let decision = if let Ok(Ok(d)) = tokio::time::timeout(self.timeout, rx).await {
            d
        } else {
            // Sender dropped (request removed) or timed out — default to deny
            self.pending.lock().await.remove(&id);
            ApprovalDecision::Deny
        };
        (id, decision)
    }

    /// Respond to a pending approval. Returns true if a pending request
    /// matched the given id and was resolved.
    pub async fn respond(&self, id: Uuid, decision: ApprovalDecision) -> bool {
        let pending = self.pending.lock().await.remove(&id);
        if let Some(p) = pending {
            let _ = p.tx.send(decision);
            true
        } else {
            false
        }
    }

    /// Get a pending approval request by id (without removing it).
    /// Used by the HTTP /approvals/:id/respond handler to capture the
    /// original session_id and tool before resolving.
    pub async fn get(&self, id: Uuid) -> Option<ApprovalRequest> {
        self.pending
            .lock()
            .await
            .get(&id)
            .map(|p| p.request.clone())
    }

    /// List pending approvals with how long each has been waiting.
    pub async fn list(&self) -> Vec<serde_json::Value> {
        self.pending
            .lock()
            .await
            .values()
            .map(|p| {
                serde_json::json!({
                    "id": p.request.id,
                    "session_id": p.request.session_id,
                    "tool": p.request.tool,
                    "args": p.request.args,
                    "reason": p.request.reason,
                    "created_at": p.request.created_at,
                    "age_ms": p.created_at.elapsed().as_millis() as u64,
                })
            })
            .collect()
    }

    /// Cancel all approvals for a given session.
    pub async fn cancel_session(&self, session_id: Uuid) {
        let mut g = self.pending.lock().await;
        let to_remove: Vec<Uuid> = g
            .iter()
            .filter(|(_, p)| p.request.session_id == session_id)
            .map(|(id, _)| *id)
            .collect();
        for id in to_remove {
            if let Some(p) = g.remove(&id) {
                let _ = p.tx.send(ApprovalDecision::Deny);
            }
        }
    }

    pub async fn cancel_all(&self) {
        let mut g = self.pending.lock().await;
        for (_, p) in g.drain() {
            let _ = p.tx.send(ApprovalDecision::Deny);
        }
    }
}

pub type SharedApprovalManager = Arc<ApprovalManager>;
