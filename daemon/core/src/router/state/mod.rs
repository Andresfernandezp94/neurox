//! `AppState` is decomposed into typed `Layer`s so handlers can
//! declare the dependencies they actually use via `state.<layer>.<field>`
//! instead of reaching into a god object.
//!
//! Layer membership:
//! - `LifecycleLayer`   — registry, supervisor, spawner, tasks,
//!                         approvals, session_agents
//! - `EventsLayer`      — event_tx, clients
//! - `AuthLayer`        — auth + orchestrator
//! - `WorkspaceLayer`   — workspace_root, sandbox, llm_catalog_cache
//! - `Engine`           — tools-engine (separate crate; treated as a layer)
//! - `Config`           — `Arc<CoreConfig>` (kept as a direct field)

pub mod auth;
pub mod events;
pub mod lifecycle;
pub mod workspace;

pub use auth::AuthLayer;
pub use events::EventsLayer;
pub use lifecycle::LifecycleLayer;
pub use workspace::WorkspaceLayer;

use std::sync::Arc;

use crate::config::CoreConfig;

#[derive(Clone)]
pub struct AppState {
    pub lifecycle: Arc<LifecycleLayer>,
    pub events: Arc<EventsLayer>,
    pub engine: Arc<tools_engine::Engine>,
    pub auth: AuthLayer,
    pub workspace: Arc<WorkspaceLayer>,
    pub config: Arc<CoreConfig>,
    /// Workspaces = entornos aislados. `None` = la API de workspaces
    /// responde 503 y toda sesion cae al sandbox global, que es el
    /// comportamiento de antes del feature.
    ///
    /// Va aparte de `workspace: WorkspaceLayer` a proposito: ese layer es
    /// el root y el sandbox GLOBAL (una sola cosa), y el nombre se
    /// prestaba a confusion. Este es el multi-tenant.
    pub workspaces: Option<Arc<crate::workspaces::WorkspacesLayer>>,
}

impl AppState {
    /// Build a new `AppState` from its layers.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lifecycle: Arc<LifecycleLayer>,
        events: Arc<EventsLayer>,
        engine: Arc<tools_engine::Engine>,
        auth: AuthLayer,
        workspace: Arc<WorkspaceLayer>,
        config: Arc<CoreConfig>,
    ) -> Self {
        Self {
            lifecycle,
            events,
            engine,
            auth,
            workspace,
            config,
            workspaces: None,
        }
    }

    /// Crea el `AppState` con workspaces. Se separa de `new` a proposito:
    /// `new` tiene seis argumentos y los tests que arman el estado a mano
    /// (los de rutas y los de `http_e2e`) no tienen por que saber de
    /// workspaces para que sus endpoints sigan andando.
    #[must_use]
    pub fn with_workspaces(
        mut self,
        workspaces: Arc<crate::workspaces::WorkspacesLayer>,
    ) -> Self {
        self.workspaces = Some(workspaces);
        self
    }
}
