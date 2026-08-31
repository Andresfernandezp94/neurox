use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::{AgentsConfig, CoreConfig, EphemeralAgentSpec, PersistentAgentSpec};

pub struct Registry {
    persistent: RwLock<HashMap<String, PersistentAgentSpec>>,
    ephemeral_templates: RwLock<HashMap<String, EphemeralAgentSpec>>,
    config_path: PathBuf,
}

impl Registry {
    #[must_use]
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            persistent: RwLock::new(HashMap::new()),
            ephemeral_templates: RwLock::new(HashMap::new()),
            config_path,
        }
    }

    pub async fn load_from_config(&self, cfg: &CoreConfig) -> anyhow::Result<()> {
        self.load_agents(&cfg.agents).await;
        info!(
            persistent = self.persistent.read().await.len(),
            ephemeral = self.ephemeral_templates.read().await.len(),
            "registry loaded"
        );
        Ok(())
    }

    async fn load_agents(&self, agents: &AgentsConfig) {
        let mut persistent = self.persistent.write().await;
        persistent.clear();
        for spec in &agents.persistent {
            persistent.insert(spec.id.clone(), spec.clone());
        }
        let mut ephemeral = self.ephemeral_templates.write().await;
        ephemeral.clear();
        for spec in &agents.ephemeral_templates {
            ephemeral.insert(spec.id.clone(), spec.clone());
        }
    }

    pub async fn register_persistent(&self, spec: PersistentAgentSpec) {
        let mut g = self.persistent.write().await;
        g.insert(spec.id.clone(), spec);
    }

    pub async fn register_ephemeral_template(&self, spec: EphemeralAgentSpec) {
        let mut g = self.ephemeral_templates.write().await;
        g.insert(spec.id.clone(), spec);
    }

    /// EP-frontend-config: replace the persistent spec for `id` if it
    /// exists. Returns the replaced spec (so the caller can echo it
    /// back to the frontend) or `None` if no such agent is registered.
    /// Does NOT create a new entry — callers should use
    /// `register_persistent` for create paths so 404 vs 200 stays
    /// unambiguous at the HTTP layer.
    pub async fn update_persistent(
        &self,
        id: &str,
        mut spec: PersistentAgentSpec,
    ) -> Option<PersistentAgentSpec> {
        spec.id = id.to_string();
        let mut g = self.persistent.write().await;
        let prev = g.get(id).cloned();
        if prev.is_some() {
            g.insert(id.to_string(), spec);
        }
        prev
    }

    /// EP-frontend-config: replace the ephemeral template spec for
    /// `id`. Same semantics as `update_persistent` — returns the
    /// previous spec on success, `None` if no such template existed.
    pub async fn update_ephemeral_template(
        &self,
        id: &str,
        mut spec: EphemeralAgentSpec,
    ) -> Option<EphemeralAgentSpec> {
        spec.id = id.to_string();
        let mut g = self.ephemeral_templates.write().await;
        let prev = g.get(id).cloned();
        if prev.is_some() {
            g.insert(id.to_string(), spec);
        }
        prev
    }

    pub async fn get_persistent(&self, id: &str) -> Option<PersistentAgentSpec> {
        self.persistent.read().await.get(id).cloned()
    }

    pub async fn get_ephemeral_template(&self, id: &str) -> Option<EphemeralAgentSpec> {
        self.ephemeral_templates.read().await.get(id).cloned()
    }

    pub async fn list_persistent(&self) -> Vec<PersistentAgentSpec> {
        self.persistent.read().await.values().cloned().collect()
    }

    pub async fn list_ephemeral_templates(&self) -> Vec<EphemeralAgentSpec> {
        self.ephemeral_templates
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    pub async fn remove_persistent(&self, id: &str) -> bool {
        self.persistent.write().await.remove(id).is_some()
    }

    pub async fn remove_ephemeral_template(&self, id: &str) -> bool {
        self.ephemeral_templates.write().await.remove(id).is_some()
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
}

pub type SharedRegistry = Arc<Registry>;
