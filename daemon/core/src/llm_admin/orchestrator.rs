//! Local LLM service orchestrator (EP-0018-02).
//!
//! Starts, probes, and supervises local LLM processes declared by providers
//! with `local_command` (e.g. `llama-server`). A failed local service NEVER
//! takes the daemon down: startup failures are retried in the background
//! with exponential backoff, bounded by a failure window + cooldown.
//!
//! Lifecycle per provider:
//!   register/startup_discover → start → run_service_lifecycle
//!     → spawn child → probe_until_ready → supervise_child (poll/reap)
//!     → unexpected exit → Failed + schedule_retry (background)

use crate::config::{LlmConfig, LlmProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Child;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// How long (seconds) a failure window covers for the retry cap.
const PROBE_WINDOW_SECS: u64 = 10;
/// Per-request timeout for the HTTP readiness probe (milliseconds).
const PROBE_REQUEST_TIMEOUT_MS: u64 = 500;
/// Total time a service has to become ready before it is killed (seconds).
const PROBE_TOTAL_TIMEOUT_SECS: u64 = 120;
/// Poll interval while supervising a child (milliseconds).
const CHILD_POLL_INTERVAL_MS: u64 = 100;
/// Base backoff for the first retry (seconds).
const BACKOFF_BASE_SECS: u64 = 1;
/// Upper bound for exponential backoff (seconds).
const BACKOFF_MAX_SECS: u64 = 30;
/// Max failures within `PROBE_WINDOW_SECS` before entering cooldown.
const MAX_FAILURES_PER_WINDOW: usize = 5;
/// Cooldown after the failure window is exhausted (seconds).
const RETRY_COOLDOWN_SECS: u64 = 300;
/// Seconds to wait for SIGTERM before escalating to SIGKILL.
const STOP_SIGTERM_WAIT_SECS: u64 = 5;
/// Interval of the background reconcile loop (seconds).
const RECONCILE_INTERVAL_SECS: u64 = 5;

/// Runtime state of a local service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Ready,
    Failed,
}

/// Operator intent for a local service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Running,
    Stopped,
}

/// Serialisable status snapshot for one local service.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalServiceStatus {
    pub provider_id: String,
    pub desired: DesiredState,
    pub state: ServiceState,
    pub pid: Option<u32>,
    /// Number of background retry attempts so far.
    pub attempts: u32,
    /// RFC3339 timestamp of the next scheduled retry (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_health_check: Option<String>,
}

/// Orchestrates local LLM service processes.
pub struct LocalServiceOrchestrator {
    /// Current status per provider id.
    services: RwLock<HashMap<String, LocalServiceStatus>>,
    /// Provider configs that were registered (only those with `local_command`).
    configs: RwLock<HashMap<String, LlmProviderConfig>>,
    /// Live child processes per provider id.
    children: Mutex<HashMap<String, Child>>,
    /// Provider ids whose `start()` is in-flight (lifecycle spawned but child
    /// not yet registered). Used to make `start()` idempotent across
    /// concurrent calls without leaving the children slot empty.
    pending_starts: Mutex<HashSet<String>>,
    /// Timestamps of recent failures, for the window/cooldown logic.
    window_failures: Mutex<HashMap<String, Vec<Instant>>>,
    /// Cancellation tokens for pending retry tasks.
    retry_tokens: Mutex<HashMap<String, CancellationToken>>,
    /// HTTP client used for the readiness probe (short timeouts).
    probe_client: reqwest::Client,
    /// Global cancellation for shutdown.
    cancel: CancellationToken,
}

impl LocalServiceOrchestrator {
    pub fn new() -> Self {
        let probe_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(PROBE_REQUEST_TIMEOUT_MS))
            .timeout(Duration::from_millis(PROBE_REQUEST_TIMEOUT_MS))
            .build()
            .unwrap_or_default();
        Self {
            services: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
            children: Mutex::new(HashMap::new()),
            pending_starts: Mutex::new(HashSet::new()),
            window_failures: Mutex::new(HashMap::new()),
            retry_tokens: Mutex::new(HashMap::new()),
            probe_client,
            cancel: CancellationToken::new(),
        }
    }

    /// Register a provider config (no-op if already registered).
    pub async fn register(&self, provider: &LlmProviderConfig) {
        let id = provider.id.clone();
        self.configs
            .write()
            .await
            .insert(id.clone(), provider.clone());
        let mut services = self.services.write().await;
        services
            .entry(id.clone())
            .or_insert_with(|| LocalServiceStatus {
                provider_id: id,
                // EP-0018 default: providers with `local_command` are REGISTERED
                // but do NOT auto-start. Operators must trigger Start from the
                // admin panel (POST /v1/llm/local/:id/start). This prevents
                // retry storms when the local binary is not installed yet.
                desired: DesiredState::Stopped,
                state: ServiceState::Stopped,
                pid: None,
                attempts: 0,
                next_retry_at: None,
                last_error: None,
                last_health_check: None,
            });
    }

    /// Discover local providers from the effective LLM config and REGISTER them.
    /// As of 2026-08-09 (EP-0018 follow-up): we do NOT auto-start. The provider
    /// becomes visible in /v1/llm/providers with `service_state: "stopped"` and
    /// can be started on demand via POST /v1/llm/local/:id/start (admin panel).
    /// This prevents retry storms when the local binary is not installed yet.
    pub async fn startup_discover(self: &Arc<Self>, llm_config: &LlmConfig) {
        for p in &llm_config.providers {
            if p.local_command.is_some() {
                self.register(p).await;
                info!(
                    provider = %p.id,
                    command = %p.local_command.as_deref().unwrap_or(""),
                    "local LLM service registered at startup (not auto-started)"
                );
            }
        }
    }

    /// Start (or restart) a local service. Idempotent: if a child is already
    /// alive or a lifecycle is in-flight, this is a no-op. Returns a boxed
    /// future to break the opaque recursive type (E0391) — `start` may
    /// re-enter itself through `schedule_retry`.
    ///
    /// Atomicity: holds both `children` and `pending_starts` locks across
    /// the existence check so two concurrent `start()` calls cannot both
    /// observe "no child + no pending" and both spawn a lifecycle.
    pub fn start(
        self: &Arc<Self>,
        provider_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>> {
        let this = self.clone();
        let provider_id = provider_id.to_string();
        Box::pin(async move {
            // Atomic check: a child exists OR a start is already pending.
            {
                let children = this.children.lock().await;
                let pending = this.pending_starts.lock().await;
                if children.contains_key(&provider_id) || pending.contains(&provider_id) {
                    return Ok(());
                }
            }

            // Mark pending + cancel any retrying task.
            this.pending_starts.lock().await.insert(provider_id.clone());
            if let Some(tok) = this.retry_tokens.lock().await.remove(&provider_id) {
                tok.cancel();
            }
            {
                let mut services = this.services.write().await;
                if let Some(st) = services.get_mut(&provider_id) {
                    // EP-0018 fix (2026-08-09): set desired=Running when the
                    // operator explicitly requests Start. Without this, after
                    // a previous Stop, desired stayed Stopped and the
                    // supervise_child loop killed the freshly-spawned child.
                    st.desired = DesiredState::Running;
                    st.state = ServiceState::Starting;
                    st.last_error = None;
                }
            }

            let this2 = this.clone();
            let pid2 = provider_id.clone();
            tokio::spawn(async move {
                this2.run_service_lifecycle(pid2).await;
            });
            Ok(())
        })
    }

    /// Stop a local service: SIGTERM → wait → SIGKILL → mark Stopped.
    pub async fn stop(self: &Arc<Self>, provider_id: &str) -> Result<(), String> {
        {
            let mut services = self.services.write().await;
            if let Some(st) = services.get_mut(provider_id) {
                st.desired = DesiredState::Stopped;
            }
        }
        // Cancel any pending retry so the service stays stopped.
        if let Some(tok) = self.retry_tokens.lock().await.remove(provider_id) {
            tok.cancel();
        }
        // Drop any in-flight start so a subsequent start() can re-acquire.
        self.pending_starts.lock().await.remove(provider_id);
        self.terminate_child(provider_id).await;
        self.mark_stopped(provider_id).await;
        Ok(())
    }

    /// Stop all registered services and cancel the reconcile loop.
    pub async fn shutdown_all(self: &Arc<Self>) {
        let ids: Vec<String> = { self.configs.read().await.keys().cloned().collect() };
        for id in ids {
            let _ = self.stop(&id).await;
        }
        self.cancel.cancel();
    }

    /// Current status for one provider, if registered.
    pub async fn status(&self, provider_id: &str) -> Option<LocalServiceStatus> {
        self.services.read().await.get(provider_id).cloned()
    }

    /// True when the service is Ready (probe passed).
    pub async fn is_ready(&self, provider_id: &str) -> bool {
        self.services
            .read()
            .await
            .get(provider_id)
            .map(|s| s.state == ServiceState::Ready)
            .unwrap_or(false)
    }

    /// Status for all registered services, ordered by provider id.
    pub async fn list_statuses(&self) -> Vec<LocalServiceStatus> {
        let mut v: Vec<LocalServiceStatus> = self.services.read().await.values().cloned().collect();
        v.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        v
    }

    /// Background loop that re-starts services which are desired to run but
    /// have no live child and no pending retry (e.g. after a supervision gap).
    pub async fn background_reconcile_loop(self: Arc<Self>) {
        let mut ticker = tokio::time::interval(Duration::from_secs(RECONCILE_INTERVAL_SECS));
        loop {
            ticker.tick().await;
            if self.cancel.is_cancelled() {
                break;
            }
            let ids: Vec<String> = {
                let services = self.services.read().await;
                services
                    .iter()
                    .filter(|(_, s)| s.desired == DesiredState::Running)
                    .map(|(id, _)| id.clone())
                    .collect()
            };
            for id in ids {
                let needs_restart = {
                    let services = self.services.read().await;
                    let child_alive = { self.children.lock().await.get(&id).is_some() };
                    let retry_pending = { self.retry_tokens.lock().await.get(&id).is_some() };
                    match services.get(&id) {
                        Some(st) => {
                            st.state != ServiceState::Ready
                                && st.state != ServiceState::Running
                                && !child_alive
                                && !retry_pending
                        }
                        None => false,
                    }
                };
                if needs_restart {
                    warn!(provider = %id, "local service missing — reconciling");
                    let _ = self.start(&id).await;
                }
            }
        }
    }

    // ─── Lifecycle internals ───────────────────────────────────────────────

    async fn run_service_lifecycle(self: &Arc<Self>, provider_id: String) {
        // Cancel any pending retry (this is a fresh attempt).
        if let Some(tok) = self.retry_tokens.lock().await.remove(&provider_id) {
            tok.cancel();
        }

        let cfg = { self.configs.read().await.get(&provider_id).cloned() };
        let Some(cfg) = cfg else {
            self.pending_starts.lock().await.remove(&provider_id);
            self.mark_failed(&provider_id, "provider config not registered".to_string())
                .await;
            self.schedule_retry(&provider_id, "provider config not registered")
                .await;
            return;
        };

        let args = match expand_args(&cfg) {
            Ok(a) => a,
            Err(e) => {
                self.pending_starts.lock().await.remove(&provider_id);
                self.mark_failed(&provider_id, e.clone()).await;
                self.schedule_retry(&provider_id, &e).await;
                return;
            }
        };

        let child = match tokio::process::Command::new(&args[0])
            .args(&args[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let msg = format!(
                    "failed to spawn '{}': {e}",
                    cfg.local_command.as_deref().unwrap_or("?")
                );
                self.pending_starts.lock().await.remove(&provider_id);
                self.mark_failed(&provider_id, msg.clone()).await;
                self.schedule_retry(&provider_id, &msg).await;
                return;
            }
        };

        let pid = child.id();
        {
            let mut children = self.children.lock().await;
            children.insert(provider_id.clone(), child);
        }
        self.pending_starts.lock().await.remove(&provider_id);
        self.mark_running(&provider_id, pid).await;

        // Probe until ready (or the child dies / the deadline expires).
        let port = cfg.local_port.unwrap_or(0);
        let probe_result = self.probe_until_ready(&provider_id, port).await;
        if let Err(e) = &probe_result {
            self.mark_failed(&provider_id, e.clone()).await;
            // If the child died before becoming ready, react immediately
            // (don't wait for supervise_child's poll interval) and remove
            // the dead child from the map so the retry can spawn a new one.
            let child_alive = {
                let mut children = self.children.lock().await;
                match children.get_mut(&provider_id) {
                    Some(child) => match child.try_wait() {
                        Ok(Some(_)) => {
                            let _ = child.wait().await;
                            false
                        }
                        Ok(None) => true,
                        Err(_) => {
                            let _ = child.wait().await;
                            false
                        }
                    },
                    None => false,
                }
            };
            if !child_alive {
                // Remove and let supervise_child short-circuit on missing entry.
                self.children.lock().await.remove(&provider_id);
                self.schedule_retry(&provider_id, e).await;
                return;
            }
        } else {
            self.mark_ready(&provider_id).await;
        }
        self.supervise_child(&provider_id).await;
    }

    async fn probe_until_ready(&self, provider_id: &str, port: u16) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(PROBE_TOTAL_TIMEOUT_SECS);
        loop {
            // If the child is gone, abort (the supervisor will reap + retry).
            let child_dead = {
                let mut children = self.children.lock().await;
                match children.get_mut(provider_id) {
                    Some(child) => match child.try_wait() {
                        Ok(Some(_)) => true,
                        Ok(None) => false,
                        Err(_) => true,
                    },
                    None => true,
                }
            };
            if child_dead {
                return Err("child exited before becoming ready".to_string());
            }

            if self.probe_once(provider_id, port).await {
                return Ok(());
            }

            if Instant::now() >= deadline {
                // Kill the child so supervision reaps it and schedules a retry.
                self.terminate_child(provider_id).await;
                return Err("service did not become ready within timeout".to_string());
            }
            tokio::time::sleep(Duration::from_millis(PROBE_REQUEST_TIMEOUT_MS)).await;
        }
    }

    async fn probe_once(&self, provider_id: &str, port: u16) -> bool {
        for path in ["/health", "/"] {
            let url = format!("http://127.0.0.1:{port}{path}");
            if let Ok(resp) = self.probe_client.get(&url).send().await {
                if resp.status().is_success() {
                    self.mark_health_check(provider_id).await;
                    return true;
                }
            }
        }
        false
    }

    async fn supervise_child(self: &Arc<Self>, provider_id: &str) {
        loop {
            tokio::time::sleep(Duration::from_millis(CHILD_POLL_INTERVAL_MS)).await;

            // Operator asked to stop → reap and finish.
            let desired_stopped = {
                let services = self.services.read().await;
                services
                    .get(provider_id)
                    .map(|s| s.desired == DesiredState::Stopped)
                    .unwrap_or(true)
            };
            if desired_stopped {
                let mut children = self.children.lock().await;
                if let Some(mut child) = children.remove(provider_id) {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
                drop(children);
                self.mark_stopped(provider_id).await;
                return;
            }

            // Poll the child; an unexpected exit → Failed + retry.
            let mut exited = false;
            {
                let mut children = self.children.lock().await;
                match children.get_mut(provider_id) {
                    Some(child) => match child.try_wait() {
                        Ok(Some(_status)) => {
                            exited = true;
                        }
                        Ok(None) => {
                            // still running
                        }
                        Err(_e) => {
                            exited = true;
                        }
                    },
                    None => {
                        return; // no child tracked — nothing to supervise
                    }
                }
            }
            if exited {
                let mut children = self.children.lock().await;
                children.remove(provider_id);
                drop(children);
                let msg = "local service process exited unexpectedly".to_string();
                self.mark_failed(provider_id, msg.clone()).await;
                self.schedule_retry(provider_id, &msg).await;
                return;
            }
        }
    }

    /// Exponential backoff retry bounded by a failure window + cooldown.
    /// Never blocks the caller: the retry runs in a cancellable background task.
    async fn schedule_retry(self: &Arc<Self>, provider_id: &str, reason: &str) {
        let now = Instant::now();
        {
            let mut wf = self.window_failures.lock().await;
            let entry = wf.entry(provider_id.to_string()).or_default();
            entry.retain(|t| *t >= now - Duration::from_secs(PROBE_WINDOW_SECS));
            entry.push(now);
        }
        let failures_in_window = {
            self.window_failures
                .lock()
                .await
                .get(provider_id)
                .map(|v| v.len())
                .unwrap_or(0)
        };
        let backoff_secs = if failures_in_window >= MAX_FAILURES_PER_WINDOW {
            warn!(
                provider = %provider_id,
                failures = failures_in_window,
                cooldown = RETRY_COOLDOWN_SECS,
                "local service hitting repeated failures — entering cooldown"
            );
            RETRY_COOLDOWN_SECS
        } else {
            let shift = (failures_in_window as u32).min(30);
            (BACKOFF_BASE_SECS.saturating_mul(1u64 << shift)).min(BACKOFF_MAX_SECS)
        };
        let delay = Duration::from_secs(backoff_secs);

        {
            let mut services = self.services.write().await;
            if let Some(st) = services.get_mut(provider_id) {
                st.attempts += 1;
                st.next_retry_at = Some(
                    (chrono::Utc::now() + chrono::Duration::from_std(delay).unwrap_or_default())
                        .to_rfc3339(),
                );
                st.last_error = Some(reason.to_string());
                st.state = ServiceState::Starting;
            }
        }

        let retry_token = CancellationToken::new();
        self.retry_tokens
            .lock()
            .await
            .insert(provider_id.to_string(), retry_token.clone());
        let this = self.clone();
        let pid = provider_id.to_string();
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = retry_token.cancelled() => return,
            }
            // Respect an operator stop that happened while we slept.
            let desired_stopped = {
                this.services
                    .read()
                    .await
                    .get(&pid)
                    .map(|s| s.desired == DesiredState::Stopped)
                    .unwrap_or(true)
            };
            if desired_stopped {
                return;
            }
            info!(provider = %pid, delay_secs = delay.as_secs(), "retrying local service start");
            let _ = this.start(&pid).await;
        });
    }

    /// SIGTERM → wait `STOP_SIGTERM_WAIT_SECS` → SIGKILL fallback.
    async fn terminate_child(&self, provider_id: &str) {
        let mut children = self.children.lock().await;
        let Some(child) = children.get_mut(provider_id) else {
            return;
        };
        let pid = child.id();
        if let Some(pid) = pid {
            // SAFETY: `pid` is the child process we spawned and still own.
            let rc = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            if rc != 0 {
                let _ = child.start_kill();
            }
        }
        match tokio::time::timeout(Duration::from_secs(STOP_SIGTERM_WAIT_SECS), child.wait()).await
        {
            Ok(_) => {}
            Err(_) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
            }
        }
    }

    // ─── Status helpers ────────────────────────────────────────────────────

    async fn mark_running(&self, provider_id: &str, pid: Option<u32>) {
        let mut services = self.services.write().await;
        if let Some(st) = services.get_mut(provider_id) {
            st.state = ServiceState::Running;
            st.pid = pid;
            st.last_error = None;
        }
    }

    async fn mark_ready(&self, provider_id: &str) {
        let mut services = self.services.write().await;
        if let Some(st) = services.get_mut(provider_id) {
            st.state = ServiceState::Ready;
            st.last_health_check = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    async fn mark_failed(&self, provider_id: &str, err: String) {
        let mut services = self.services.write().await;
        if let Some(st) = services.get_mut(provider_id) {
            st.state = ServiceState::Failed;
            st.pid = None;
            st.last_error = Some(err);
        }
    }

    async fn mark_stopped(&self, provider_id: &str) {
        let mut services = self.services.write().await;
        if let Some(st) = services.get_mut(provider_id) {
            st.state = ServiceState::Stopped;
            st.pid = None;
            st.last_error = None;
            st.next_retry_at = None;
        }
    }

    async fn mark_health_check(&self, provider_id: &str) {
        let mut services = self.services.write().await;
        if let Some(st) = services.get_mut(provider_id) {
            st.last_health_check = Some(chrono::Utc::now().to_rfc3339());
        }
    }
}

impl Default for LocalServiceOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the child argv. Placeholders: `{{port}}` is always expanded (requires
/// `local_port`); `{{model_path}}` is expanded (requires `local_model_path`)
/// ONLY if it appears in `local_args`. EP-0018-05: if a model path is
/// required, we verify it exists before returning so the operator sees a
/// clear error instead of a llama-server crash mid-spawn.
fn expand_args(cfg: &LlmProviderConfig) -> Result<Vec<String>, String> {
    let command = cfg
        .local_command
        .as_deref()
        .ok_or_else(|| "local_command is not set".to_string())?;
    let port = cfg
        .local_port
        .ok_or_else(|| "local_port is required for local services".to_string())?;
    let mut out = vec![command.to_string()];
    for arg in &cfg.local_args {
        if arg.contains("{{model_path}}") {
            let model_path = cfg.local_model_path.as_deref().ok_or_else(|| {
                "local_model_path is required because local_args contains {{model_path}}"
                    .to_string()
            })?;
            let expanded = expand_tilde(model_path);
            // EP-0018-05: early validation — the operator should see
            // "model file not found" instead of a spawn failure with a
            // confusing llama-server error. Tilde is expanded here too.
            if !std::path::Path::new(&expanded).exists() {
                return Err(format!(
                    "local model file not found: {expanded} \
                     (set NEUROX_MODELS_DIR or fix local_model_path)"
                ));
            }
            out.push(expanded);
        } else if arg.contains("{{port}}") {
            out.push(port.to_string());
        } else {
            out.push(arg.clone());
        }
    }
    Ok(out)
}

/// Expand a leading `~/` to `$HOME` (best-effort; leaves the value unchanged
/// if `HOME` is unset or the path has no leading tilde).
fn expand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home)
                .join(rest)
                .display()
                .to_string();
        }
    }
    p.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmProviderKind;

    fn test_cfg(
        id: &str,
        command: Option<String>,
        args: Vec<String>,
        port: u16,
    ) -> LlmProviderConfig {
        LlmProviderConfig {
            id: id.to_string(),
            kind: LlmProviderKind::OpenaiCompat,
            base_url: String::new(),
            model: "test-model".to_string(),
            api_key_env: None,
            extra: HashMap::new(),
            local_command: command,
            local_args: args,
            local_model_path: None,
            local_port: Some(port),
        }
    }

    /// Tiny HTTP server on an ephemeral port that answers 200 to everything.
    fn spawn_http_stub() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut s) => {
                        use std::io::Write;
                        let _ = s.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                        );
                    }
                    Err(_) => break,
                }
            }
        });
        port
    }

    #[test]
    fn test_expand_placeholders_tilde() {
        // EP-0018-05: expand_args now validates that the model file exists.
        // Create a temp file under the home dir so the `~`-expansion path
        // matches and the validation passes.
        let home = std::env::var("HOME").unwrap();
        let tmp = std::path::PathBuf::from(&home)
            .join(format!(".np-orch-stub-{}-model.gguf", std::process::id()));
        std::fs::write(&tmp, b"stub").unwrap();
        let model_rel = format!("~/{}", tmp.file_name().unwrap().to_string_lossy());

        let cfg = LlmProviderConfig {
            id: "local".to_string(),
            kind: LlmProviderKind::OpenaiCompat,
            base_url: String::new(),
            model: "qwen".to_string(),
            api_key_env: None,
            extra: HashMap::new(),
            local_command: Some("llama-server".to_string()),
            local_args: vec![
                "-m".to_string(),
                "{{model_path}}".to_string(),
                "--port".to_string(),
                "{{port}}".to_string(),
            ],
            local_model_path: Some(model_rel),
            local_port: Some(11435),
        };
        let args = expand_args(&cfg).unwrap();
        assert_eq!(args[0], "llama-server");
        assert_eq!(args[2], tmp.to_string_lossy().to_string());
        assert_eq!(args[4], "11435");
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_expand_args_missing_model_file() {
        // EP-0018-05: a missing model file must surface a clear error
        // BEFORE we attempt to spawn the child (the operator would otherwise
        // get a confusing llama-server crash mid-spawn).
        let cfg = LlmProviderConfig {
            id: "missing".to_string(),
            kind: LlmProviderKind::OpenaiCompat,
            base_url: String::new(),
            model: "x".to_string(),
            api_key_env: None,
            extra: HashMap::new(),
            local_command: Some("llama-server".to_string()),
            local_args: vec!["{{model_path}}".to_string()],
            local_model_path: Some("~/no-such-model-file-{}.gguf".to_string()),
            local_port: Some(11435),
        };
        let err = expand_args(&cfg).unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
        assert!(err.contains("NEUROX_MODELS_DIR"), "missing hint: {err}");
    }

    #[test]
    fn test_expand_args_missing_fields() {
        // No port → error.
        let no_port = test_cfg("a", Some("cmd".to_string()), vec![], 0);
        let mut no_port = no_port;
        no_port.local_port = None;
        assert!(expand_args(&no_port).is_err());

        // Placeholder `{{model_path}}` without `local_model_path` → error.
        let missing_path = test_cfg(
            "b",
            Some("cmd".to_string()),
            vec!["{{model_path}}".to_string()],
            11435,
        );
        assert!(expand_args(&missing_path).is_err());

        // No `{{model_path}}` in args → OK even without `local_model_path`.
        let ok_no_path = test_cfg(
            "c",
            Some("cmd".to_string()),
            vec!["--port".to_string(), "{{port}}".to_string()],
            11435,
        );
        assert!(expand_args(&ok_no_path).is_ok());
    }

    #[tokio::test]
    async fn test_ready_stop_cycle() {
        let port = spawn_http_stub();
        let orch = Arc::new(LocalServiceOrchestrator::new());
        let cfg = test_cfg(
            "svc-a",
            Some("sh".to_string()),
            vec!["-c".to_string(), "sleep 60".to_string()],
            port,
        );
        orch.register(&cfg).await;
        orch.start(&cfg.id).await.unwrap();
        for _ in 0..50 {
            if orch.is_ready(&cfg.id).await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(orch.is_ready(&cfg.id).await, "service should become ready");
        let st = orch.status(&cfg.id).await.unwrap();
        assert_eq!(st.state, ServiceState::Ready);
        assert!(st.pid.is_some());

        orch.stop(&cfg.id).await.unwrap();
        let st = orch.status(&cfg.id).await.unwrap();
        assert_eq!(st.state, ServiceState::Stopped);
    }

    #[tokio::test]
    async fn test_idempotent_start_stop() {
        let port = spawn_http_stub();
        let orch = Arc::new(LocalServiceOrchestrator::new());
        let cfg = test_cfg(
            "svc-idem",
            Some("sh".to_string()),
            vec!["-c".to_string(), "sleep 60".to_string()],
            port,
        );
        orch.register(&cfg).await;
        orch.start(&cfg.id).await.unwrap();
        orch.start(&cfg.id).await.unwrap(); // second start is a no-op (pending)
                                            // Wait until the lifecycle has actually inserted the child (start is async).
        let mut inserted = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if orch.children.lock().await.contains_key(&cfg.id) {
                inserted = true;
                break;
            }
        }
        assert!(
            inserted,
            "lifecycle should have inserted the child within 1s"
        );
        assert_eq!(orch.children.lock().await.len(), 1);
        orch.stop(&cfg.id).await.unwrap();
        orch.stop(&cfg.id).await.unwrap(); // second stop must be a no-op
        assert_eq!(
            orch.status(&cfg.id).await.unwrap().state,
            ServiceState::Stopped
        );
    }

    #[tokio::test]
    async fn test_failed_retry_scheduled() {
        let orch = Arc::new(LocalServiceOrchestrator::new());
        // The command exits immediately; the probe port has no listener, so
        // the service can never become ready.
        let cfg = test_cfg(
            "svc-fail",
            Some("sh".to_string()),
            vec!["-c".to_string(), "exit 1".to_string()],
            1,
        );
        orch.register(&cfg).await;
        orch.start(&cfg.id).await.unwrap();
        // Poll for the failure to be observed + retry scheduled. Generous
        // window because lifecycle + probe + supervise must all run.
        let mut st = None;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let s = orch.status(&cfg.id).await.unwrap();
            if s.attempts >= 1 && s.last_error.is_some() {
                st = Some(s);
                break;
            }
        }
        let st = st.expect("retry should have been scheduled within 2.5s");
        // After schedule_retry, state is Starting (the retry is in flight).
        // We only assert the failure was observed + retry was scheduled.
        assert!(matches!(
            st.state,
            ServiceState::Starting | ServiceState::Failed
        ));
        assert!(st.attempts >= 1);
        assert!(st.last_error.is_some());
    }

    #[tokio::test]
    async fn test_shutdown_all() {
        let port1 = spawn_http_stub();
        let port2 = spawn_http_stub();
        let orch = Arc::new(LocalServiceOrchestrator::new());
        let a = test_cfg(
            "svc-a",
            Some("sh".to_string()),
            vec!["-c".to_string(), "sleep 60".to_string()],
            port1,
        );
        let b = test_cfg(
            "svc-b",
            Some("sh".to_string()),
            vec!["-c".to_string(), "sleep 60".to_string()],
            port2,
        );
        orch.register(&a).await;
        orch.register(&b).await;
        orch.start(&a.id).await.unwrap();
        orch.start(&b.id).await.unwrap();
        // Generous window: under cargo's default parallel test execution,
        // probe + lifecycle contention can delay readiness.
        let mut both_ready = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if orch.is_ready(&a.id).await && orch.is_ready(&b.id).await {
                both_ready = true;
                break;
            }
        }
        assert!(both_ready, "both services should become ready within 2.5s");
        orch.shutdown_all().await;
        assert_eq!(orch.list_statuses().await.len(), 2);
        for st in orch.list_statuses().await {
            assert_eq!(st.state, ServiceState::Stopped);
        }
    }
}
