//! Shared test helpers for `core/tests/`.
//!
//! Provides `build_app_state` to construct an `AppState` from the
//! primitives most integration tests already build (registry,
//! supervisor, session store, tool registry, etc.) using the new
//! layered constructor from EP-0004 wave 4.
//!
//! Tests use this via:
//!   ```ignore
//!   use crate::common::build_app_state;
//!   ```

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use neurox::approval::ApprovalManager;
use neurox::config::CoreConfig;
use neurox::router::state::{
    AppState, AuthLayer, EventsLayer, LifecycleLayer, WorkspaceLayer,
};
use neurox::registry::Registry;
use neurox::session::SessionStore;
use neurox::session_agents::SessionAgentPool;
use neurox::skills::SkillsRegistry;
use neurox::spawner::Spawner;
use neurox::supervisor::Supervisor;
use neurox::tasks::TaskManager;

/// Build a complete `AppState` for integration tests. Mirrors the
/// production wiring but with empty defaults. `db_path` is the path
/// where the session store will live; pass a tempfile-generated path.
///
/// Auth is JWT-only since EP-2026-08-19 (the legacy `api_token` was
/// removed). Tests that need auth-enabled state should use
/// `AuthLayer::with_auth(...)` after construction.
pub async fn build_app_state(
    db_path: PathBuf,
    tools: Arc<tools_engine::tools::ToolRegistry>,
    workspace_root: PathBuf,
) -> AppState {
    let registry = Arc::new(Registry::new(workspace_root.clone()));
    let session = Arc::new(
        SessionStore::open(&db_path)
            .await
            .expect("session store open"),
    );
    let engine = tools_engine::Engine::for_testing(
        tools.clone(),
        workspace_root.clone(),
        Arc::new(tokio::sync::RwLock::new(Box::new(tools_engine::DefaultSandbox) as Box<dyn tools_engine::SandboxConfig>)),
    )
    .await
    .expect("engine for testing");

    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let lifecycle = Arc::new(LifecycleLayer::new(
        registry,
        Arc::new(Supervisor::new()),
        Arc::new(Spawner::new(8)),
        Arc::new(TaskManager::new()),
        Arc::new(ApprovalManager::default()),
        Arc::new(SessionAgentPool::default()),
        session,
        Arc::new(SkillsRegistry::new()),
    ));
    let auth = AuthLayer::new();
    let workspace = Arc::new(WorkspaceLayer::new(
        workspace_root,
        Arc::new(tokio::sync::RwLock::new(Box::new(neurox::config::SandboxConfig::default()) as Box<dyn tools_engine::SandboxConfig>)),
    ));

    AppState::new(
        lifecycle,
        Arc::new(EventsLayer::new(event_tx)),
        engine,
        auth,
        workspace,
        Arc::new(CoreConfig::default()),
    )
}

/// Like [`build_app_state`] but auto-generates a unique temp dir for
/// the session store. Useful when the test doesn't otherwise need a
/// `tempfile::TempDir`.
pub async fn build_app_state_auto(
    tools: Arc<tools_engine::tools::ToolRegistry>,
    workspace_root: PathBuf,
) -> AppState {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp
        .path()
        .join(format!("neurox-test-{}.db", uuid::Uuid::new_v4()));
    // tmp is dropped here but the SQLite handle keeps the file alive
    // for the duration of the process.
    build_app_state(db_path, tools, workspace_root).await
}
