use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::EphemeralAgentSpec;
use crate::events::Event;
use crate::protocols::SharedProtocol;

pub struct Spawner {
    running: RwLock<HashMap<String, EphemeralHandle>>,
    concurrency_limit: usize,
}

struct EphemeralHandle {
    spec_id: String,
    ephemeral_id: String,
    session_id: Uuid,
    task: JoinHandle<()>,
    started_at: std::time::Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EphemeralInfo {
    pub ephemeral_id: String,
    pub spec_id: String,
    pub session_id: Uuid,
    pub elapsed_ms: u64,
}

impl Spawner {
    #[must_use]
    pub fn new(concurrency_limit: usize) -> Self {
        Self {
            running: RwLock::new(HashMap::new()),
            concurrency_limit,
        }
    }

    pub async fn spawn<F, Fut>(
        self: &Arc<Self>,
        _template_id: &str,
        template: EphemeralAgentSpec,
        session_id: Uuid,
        tx: tokio::sync::mpsc::Sender<Event>,
        work: F,
    ) -> anyhow::Result<String>
    where
        F: FnOnce(SharedProtocol, tokio::sync::mpsc::Sender<Event>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        if self.running.read().await.len() >= self.concurrency_limit {
            anyhow::bail!(
                "concurrency limit reached ({}), retry later",
                self.concurrency_limit
            );
        }

        let ephemeral_id = Uuid::new_v4().to_string();
        let protocol = self.spawn_protocol(&template).await?;

        let spawner = Arc::clone(self);
        let ephemeral_id_clone = ephemeral_id.clone();
        let spec_id_clone = template.id.clone();

        let task = tokio::spawn(async move {
            let started = std::time::Instant::now();
            info!(
                ephemeral_id = %ephemeral_id_clone,
                agent = %spec_id_clone,
                "ephemeral started"
            );

            let result = work(protocol.clone(), tx).await;

            let elapsed = started.elapsed();
            let status = if result.is_ok() { "ok" } else { "error" };
            info!(
                ephemeral_id = %ephemeral_id_clone,
                agent = %spec_id_clone,
                status,
                elapsed_ms = elapsed.as_millis() as u64,
                "ephemeral finished"
            );

            spawner.running.write().await.remove(&ephemeral_id_clone);

            let _ = protocol.shutdown().await;
        });

        let handle = EphemeralHandle {
            spec_id: template.id,
            ephemeral_id: ephemeral_id.clone(),
            session_id,
            task,
            started_at: std::time::Instant::now(),
        };

        self.running
            .write()
            .await
            .insert(ephemeral_id.clone(), handle);
        Ok(ephemeral_id)
    }

    async fn spawn_protocol(&self, spec: &EphemeralAgentSpec) -> anyhow::Result<SharedProtocol> {
        use crate::config::AgentKind;
        use crate::protocols::{ProtocolKind, TransportKind};

        let AgentKind::Subprocess { command, args, env } = &spec.kind;
        if !matches!(spec.protocol, ProtocolKind::JsonRpc)
            || !matches!(spec.transport, TransportKind::Stdio)
        {
            anyhow::bail!("unsupported ephemeral agent protocol/transport");
        }
        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(env.iter())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);
        let child = cmd.spawn()?;
        Ok(Arc::new(
            crate::protocols::json_rpc_stdio::JsonRpcStdio::new(child),
        ))
    }

    pub async fn running_count(&self) -> usize {
        self.running.read().await.len()
    }

    pub async fn list(&self) -> Vec<EphemeralInfo> {
        let running = self.running.read().await;
        running
            .values()
            .map(|h| EphemeralInfo {
                ephemeral_id: h.ephemeral_id.clone(),
                spec_id: h.spec_id.clone(),
                session_id: h.session_id,
                elapsed_ms: h.started_at.elapsed().as_millis() as u64,
            })
            .collect()
    }

    pub async fn cancel(&self, ephemeral_id: &str) -> anyhow::Result<()> {
        let mut running = self.running.write().await;
        if let Some(handle) = running.remove(ephemeral_id) {
            handle.task.abort();
            let _ = handle.task.await;
            Ok(())
        } else {
            anyhow::bail!("ephemeral agent not found: {ephemeral_id}")
        }
    }

    pub async fn shutdown_all(self: Arc<Self>, timeout: Duration) {
        info!("shutting down all ephemeral agents");
        let ids: Vec<String> = {
            let running = self.running.read().await;
            running.keys().cloned().collect()
        };

        for id in ids {
            if let Err(e) = self.cancel(&id).await {
                warn!(ephemeral_id = %id, error = %e, "cancel error");
            }
        }

        let deadline = std::time::Instant::now() + timeout;
        while self.running_count().await > 0 && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Default for Spawner {
    fn default() -> Self {
        Self::new(8)
    }
}
