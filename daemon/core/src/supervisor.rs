use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::config::{PersistentAgentSpec, RestartPolicy};
use crate::protocols::SharedProtocol;

const MAX_RESTARTS_PER_HOUR: u32 = 10;
const RESTART_WINDOW: Duration = Duration::from_secs(3600);

pub struct Supervisor {
    agents: Arc<RwLock<HashMap<String, ManagedAgent>>>,
    /// EP-0013 T-004: per-agent restart counts within `RESTART_WINDOW`.
    /// If a count exceeds `MAX_RESTARTS_PER_HOUR`, the agent is
    /// disabled (no more respawns until the window expires).
    restart_counts: Arc<RwLock<HashMap<String, RestartCounter>>>,
}

struct RestartCounter {
    count: u32,
    window_start: Instant,
}

struct ManagedAgent {
    spec: PersistentAgentSpec,
    protocol: SharedProtocol,
}

impl Supervisor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            restart_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_agent(&self, spec: PersistentAgentSpec) -> anyhow::Result<()> {
        info!(agent_id = %spec.id, "starting persistent agent");
        let protocol = self.spawn_agent(&spec).await?;
        let agent_id = spec.id.clone();
        let mut agents = self.agents.write().await;
        agents.insert(agent_id.clone(), ManagedAgent { spec: spec.clone(), protocol: protocol.clone() });
        drop(agents);
        // EP-0013 T-004: spawn watch_loop for restart policy.
        self.spawn_watch_loop(agent_id, spec, protocol);
        Ok(())
    }

    /// EP-0013 T-004: watch_loop watches the agent subprocess. When it
    /// exits, applies `RestartPolicy`:
    /// - `Always`: respawn unconditionally (subject to circuit breaker)
    /// - `OnFailure`: respawn only if exit code != 0
    /// - `Never`: don't respawn
    fn spawn_watch_loop(&self, agent_id: String, spec: PersistentAgentSpec, protocol: SharedProtocol) {
        let counts = self.restart_counts.clone();
        let agents = self.agents.clone();
        tokio::spawn(async move {
            loop {
                let child_opt = protocol.take_child().await;
                let Some(mut child) = child_opt else {
                    // Protocol was shut down. Stop watching.
                    return;
                };
                let status = match child.wait().await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(agent_id = %agent_id, error = %e, "child.wait() failed");
                        return;
                    }
                };
                let code = status.code();
                let success = status.success();
                info!(
                    agent_id = %agent_id,
                    code = ?code,
                    success = success,
                    "persistent agent exited"
                );

                // Decide whether to restart.
                let should_restart = match spec.restart_policy {
                    RestartPolicy::Always => true,
                    RestartPolicy::OnFailure => !success,
                    RestartPolicy::Never => false,
                };

                if !should_restart {
                    // Remove from registry so future calls return None.
                    let mut a = agents.write().await;
                    a.remove(&agent_id);
                    return;
                }

                // Circuit breaker.
                let mut counts_g = counts.write().await;
                let now = Instant::now();
                let entry = counts_g.entry(agent_id.clone()).or_insert(RestartCounter {
                    count: 0,
                    window_start: now,
                });
                if now.duration_since(entry.window_start) > RESTART_WINDOW {
                    entry.count = 0;
                    entry.window_start = now;
                }
                entry.count += 1;
                if entry.count > MAX_RESTARTS_PER_HOUR {
                    error!(
                        agent_id = %agent_id,
                        count = entry.count,
                        "max restarts/hour reached, disabling agent"
                    );
                    let mut a = agents.write().await;
                    a.remove(&agent_id);
                    return;
                }
                drop(counts_g);

                // Respawn.
                warn!(agent_id = %agent_id, "respawning agent");
                let new_protocol = match spawn_via_spec(&spec).await {
                    Ok(p) => p,
                    Err(e) => {
                        error!(agent_id = %agent_id, error = %e, "respawn failed");
                        let mut a = agents.write().await;
                        a.remove(&agent_id);
                        return;
                    }
                };
                let mut a = agents.write().await;
                if let Some(existing) = a.get_mut(&agent_id) {
                    existing.protocol = new_protocol;
                }
                // Loop: next iteration takes the new protocol's child.
            }
        });
    }

    async fn spawn_agent(&self, spec: &PersistentAgentSpec) -> anyhow::Result<SharedProtocol> {
        spawn_via_spec(spec).await
    }

    pub async fn get_protocol(&self, id: &str) -> Option<SharedProtocol> {
        let agents = self.agents.read().await;
        agents.get(id).map(|a| a.protocol.clone())
    }

    pub async fn stop_agent(&self, id: &str) -> anyhow::Result<()> {
        info!(agent_id = %id, "stopping agent");
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.remove(id) {
            if let Err(e) = agent.protocol.shutdown().await {
                warn!(agent_id = %id, error = %e, "shutdown error (non-fatal)");
            }
            Ok(())
        } else {
            anyhow::bail!("agent not found: {id}")
        }
    }

    pub async fn list(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read().await;
        let mut out = Vec::new();
        for (id, managed) in agents.iter() {
            let health = managed.protocol.health().await;
            out.push(AgentInfo {
                id: id.clone(),
                status: format!("{:?}", health.status),
                detail: health.detail,
                // Redact env values — the supervisor's spec is exposed verbatim
                // by /v1/agents and would otherwise leak API keys (B12).
                spec: Some(managed.spec.redact_secrets()),
            });
        }
        out
    }

    pub async fn shutdown_all(&self) {
        info!("shutting down all persistent agents");
        let mut agents = self.agents.write().await;
        let ids: Vec<String> = agents.keys().cloned().collect();
        for id in ids {
            if let Some(agent) = agents.remove(&id) {
                if let Err(e) = agent.protocol.shutdown().await {
                    error!(agent_id = %id, error = %e, "shutdown error");
                }
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub status: String,
    pub detail: Option<String>,
    pub spec: Option<PersistentAgentSpec>,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

/// EP-0013 T-004: free function form of `spawn_agent` so the watch_loop
/// can respawn without borrowing `&self`.
async fn spawn_via_spec(spec: &PersistentAgentSpec) -> anyhow::Result<SharedProtocol> {
    use crate::config::AgentKind;
    use crate::protocols::{ProtocolKind, TransportKind};

    let AgentKind::Subprocess { command, args, env } = &spec.kind;
    if !matches!(spec.protocol, ProtocolKind::JsonRpc)
        || !matches!(spec.transport, TransportKind::Stdio)
    {
        anyhow::bail!(
            "unsupported agent protocol/transport for {}: must be json-rpc+stdio",
            spec.id
        );
    }
    let mut cmd = Command::new(command);
    cmd.args(args)
        .envs(env.iter())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);
    let child = cmd.spawn()?;
    info!(agent_id = %spec.id, pid = child.id(), "subprocess spawned");
    Ok(Arc::new(
        crate::protocols::json_rpc_stdio::JsonRpcStdio::new(child),
    ))
}
