//! Long-lived components that own the daemon's mutable state.

use std::sync::Arc;

use crate::approval::SharedApprovalManager;
use crate::registry::SharedRegistry;
use crate::session::SessionStore;
use crate::session_agents::SessionAgentPool;
use crate::skills::SharedSkillsRegistry;
use crate::spawner::Spawner;
use crate::supervisor::Supervisor;
use crate::tasks::SharedTaskManager;

#[derive(Clone)]
pub struct LifecycleLayer {
    pub registry: SharedRegistry,
    pub supervisor: Arc<Supervisor>,
    pub spawner: Arc<Spawner>,
    pub tasks: SharedTaskManager,
    pub approvals: SharedApprovalManager,
    pub session_agents: Arc<SessionAgentPool>,
    /// SQLite-backed session/message store.
    pub session: Arc<SessionStore>,
    /// EP-frontend-config: skill enable/disable overrides.
    pub skills: SharedSkillsRegistry,
}

impl LifecycleLayer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: SharedRegistry,
        supervisor: Arc<Supervisor>,
        spawner: Arc<Spawner>,
        tasks: SharedTaskManager,
        approvals: SharedApprovalManager,
        session_agents: Arc<SessionAgentPool>,
        session: Arc<SessionStore>,
        skills: SharedSkillsRegistry,
    ) -> Self {
        Self {
            registry,
            supervisor,
            spawner,
            tasks,
            approvals,
            session_agents,
            session,
            skills,
        }
    }
}