//! Engine state — held by the daemon's `AppState` as `Arc<EngineState>`.
//!
//! Owns the SQLite-backed provider store, the local model configs table,
//! the tool registry, the sandbox config, and the auth resolver. The
//! daemon's HTTP handlers consult this directly (no more HTTP hop to a
//! separate llmd process).

use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

use sqlx::SqlitePool;

use crate::sandbox::SandboxConfig;
use crate::tools::ToolRegistry;

pub struct EngineState {
    /// When the engine was constructed. Surfaced via `GET /v1/status`.
    pub started_at: Instant,

    /// Tool registry — populated at startup via
    /// `crate::tools::register_defaults`. The daemon reads this directly
    /// for `/v1/tools` and tool execution.
    pub tools: Arc<ToolRegistry>,

    /// SQLite-backed provider CRUD. Same db_path the daemon uses for
    /// its session store, so they share a file.
    pub db: SqlitePool,

    /// Path-level allowlist applied by filesystem / shell tools.
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,

    /// Workspace root for filesystem tools (e.g. `read_file`).
    pub workspace_root: std::path::PathBuf,
}

impl EngineState {
    pub fn new(
        tools: Arc<ToolRegistry>,
        db: SqlitePool,
        sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
        workspace_root: std::path::PathBuf,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            tools,
            db,
            sandbox,
            workspace_root,
        }
    }
}
