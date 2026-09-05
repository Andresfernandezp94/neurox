//! Auth and local LLM service orchestration.

use std::sync::Arc;

use crate::auth::handlers::AuthState;
use crate::llm_admin::LocalServiceOrchestrator;

#[derive(Clone, Default)]
pub struct AuthLayer {
    /// EP-0007: JWT auth state. `None` ⇒ auth disabled.
    pub auth: Option<AuthState>,
    /// EP-0018-02: local LLM service orchestrator (llama-server etc.).
    pub orchestrator: Option<Arc<LocalServiceOrchestrator>>,
}

impl AuthLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_auth(mut self, auth: AuthState) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn with_orchestrator(mut self, orchestrator: Arc<LocalServiceOrchestrator>) -> Self {
        self.orchestrator = Some(orchestrator);
        self
    }
}