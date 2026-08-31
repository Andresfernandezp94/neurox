//! Workspace root, sandbox config, model catalog cache.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::llm_admin::catalog::ModelCatalog;

#[derive(Clone)]
pub struct WorkspaceLayer {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>>,
    pub llm_catalog_cache: Arc<tokio::sync::Mutex<Option<ModelCatalog>>>,
}

impl WorkspaceLayer {
    pub fn new(
        workspace_root: PathBuf,
        sandbox: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>>,
    ) -> Self {
        Self {
            workspace_root,
            sandbox,
            llm_catalog_cache: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}
