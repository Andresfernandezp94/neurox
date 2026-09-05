//! Session-scoped agent pool — one subprocess per chat session.
//!
//! Each chat session that targets a `session_agents` spec gets its own
//! dedicated subprocess (JSON-RPC over stdio). The process is killed when
//! the session is cancelled, deleted, evicted for idleness, or the daemon
//! shuts down. This gives real per-session parallelism: each session's
//! conversation history lives in its own process and is physically isolated
//! from every other session.
//!
//! The in-process `default` is **not** used for sessions backed by
//! a spec — that path is reserved for agents not configured here (legacy
//! fallback).
//!
//! Resource model:
//! - One Tokio child process per session (no in-process state).
//! - Idle eviction sweeper kills sessions with `last_active` older than
//!   the spec's `idle_timeout_secs` (default 30 min).
//! - Upper bound is implicit (hardware); an optional `max_sessions` per
//!   spec gives a soft cap for noisy-neighbour protection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
unsafe extern "C" {
    fn syscall(num: i64, ...) -> i64;
}

#[cfg(unix)]
const SYS_KILL: i64 = 37; // Linux x86_64. Same on arm64.

/// Probe a pid via `syscall(SYS_kill, pid, 0)`. Sending signal 0
/// doesn't deliver anything but performs an existence/permission
/// check — returns 0 if the process exists, -1 otherwise.
///
/// Using `syscall` directly instead of libc::kill avoids any Rust
/// symbol shadowing surprises.
#[cfg(unix)]
#[inline]
unsafe fn probe_pid(pid: u32) -> bool {
    syscall(SYS_KILL, pid as i64, 0i64) == 0
}
use tokio::sync::RwLock;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::{InProcessAgentSpec, SessionAgentSpec};
use crate::protocols::json_rpc_stdio::JsonRpcStdio;
use crate::protocols::SharedProtocol;

/// Live session-bound agent. Holds the protocol handle to the subprocess
/// and bookkeeping the daemon needs to expose via the API.
pub struct SessionAgent {
    pub session_id: Uuid,
    pub agent_id: String,
    pub pid: Option<u32>,
    pub protocol: SharedProtocol,
    pub started_at: Instant,
    pub last_active: Arc<parking_lot::Mutex<Instant>>,
    pub spec: SessionAgentSpec,
    /// Number of dispatch requests currently in-flight for this
    /// session. `active = inflight > 0`. Acquired by the dispatcher
    /// at the start of every request and released on completion.
    pub inflight: Arc<AtomicU64>,
    /// EP-2026-08-15 (live-switch history injection): `false` from
    /// creation until the daemon has called the subprocess's
    /// `seed_history` once with the session's DB-backed history. The
    /// daemon's `dispatch_to_session_agent` checks this flag on the
    /// first dispatch and seeds if needed; subsequent dispatches skip
    /// the seed (the subprocess already has the history in its
    /// WorkingMemory). Reset by `start_for_session` whenever a new
    /// subprocess is spawned for this session — including the live-
    /// switch path where the user changes agents mid-session.
    pub history_seeded: Arc<AtomicBool>,
}

/// RAII guard that decrements the session's `inflight` counter on
/// drop. Use this inside `dispatch_to_session_agent` so the counter
/// is always decremented even on early returns / panics.
pub struct InflightGuard {
    counter: Arc<AtomicU64>,
}

impl InflightGuard {
    pub fn new(counter: Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self { counter }
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

impl SessionAgent {
    pub fn touch(&self) {
        *self.last_active.lock() = Instant::now();
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionAgentInfo {
    pub session_id: Uuid,
    pub agent_id: String,
    pub pid: Option<u32>,
    pub command: String,
    pub args: Vec<String>,
    pub uptime_secs: u64,
    pub idle_secs: u64,
    pub max_idle_secs: u64,
    /// Inflight dispatch requests — `active = inflight > 0`.
    /// Snapshot at `list()` time (atomic load).
    pub inflight: u64,
}

pub struct SessionAgentPool {
    /// Spec registry: agent_id → how to spawn a process for a session.
    /// Behind an `RwLock` so the daemon can register/deregister
    /// specs at runtime via `register_spec` / `deregister_spec`
    /// (EP-2026-08-15 runtime agents).
    specs: RwLock<HashMap<String, SessionAgentSpec>>,
    /// Live per-session subprocesses.
    agents: Mutex<HashMap<Uuid, Arc<SessionAgent>>>,
    /// EP-0004 wave 3: persistent agent subprocesses (one per
    /// configured agent_id). Spawned once at startup; dispatched
    /// via JSON-RPC over stdio. Folded into SessionAgentPool from
    /// the legacy `core::agent_runtime` module.
    persistent: RwLock<HashMap<String, PersistentAgentHandle>>,
    /// Per-subprocess stdio locks for `send_chat_persistent`.
    persistent_locks: Mutex<PersistentIoLocks>,
}

impl SessionAgentPool {
    #[must_use]
    pub fn new(specs: HashMap<String, SessionAgentSpec>) -> Self {
        Self {
            specs: RwLock::new(specs),
            agents: Mutex::new(HashMap::new()),
            persistent: RwLock::new(HashMap::new()),
            persistent_locks: Mutex::new(PersistentIoLocks::default()),
        }
    }

    /// Returns true if `agent_id` has a per-session spec configured.
    pub async fn is_session_agent(&self, agent_id: &str) -> bool {
        self.specs.read().await.contains_key(agent_id)
    }

    /// EP-2026-08-15 (Fix 3 + 4): read-only clone of a spec by id.
    /// Used by `GET /v1/agents/:id` so the API can surface session_agent
    /// entries without spawning anything.
    pub async fn spec(&self, agent_id: &str) -> Option<SessionAgentSpec> {
        self.specs.read().await.get(agent_id).cloned()
    }

    /// List all configured spec names.
    pub async fn list_specs(&self) -> Vec<(String, SessionAgentSpec)> {
        self.specs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// EP-2026-08-15: runtime registration path. Lets the daemon
    /// accept POST /v1/agents/in_process and register a brand new
    /// session_agent spec on the fly (without restart). The spec is
    /// immediately usable: subsequent POST /v1/sessions with this
    /// `agent_id` will spawn an agent subprocess for the session.
    pub async fn register_spec(&self, id: String, spec: SessionAgentSpec) {
        self.specs.write().await.insert(id, spec);
    }

    /// EP-frontend-config: replace the spec for `id` if it exists.
    /// Returns the previous spec on success, `None` if no spec with
    /// that id was registered (callers should map that to 404).
    ///
    /// Active sessions are unaffected: their subprocesses keep the
    /// spec they were spawned with. New sessions will pick up the
    /// updated spec.
    pub async fn update_spec(
        &self,
        id: &str,
        spec: SessionAgentSpec,
    ) -> Option<SessionAgentSpec> {
        let mut g = self.specs.write().await;
        let prev = g.get(id).cloned();
        if prev.is_some() {
            g.insert(id.to_string(), spec);
        }
        prev
    }

    /// EP-2026-08-15: deregister a spec at runtime. Returns the spec
    /// that was removed so the caller can clean up identity_dir if
    /// desired. Active sessions already spawned before this call
    /// are unaffected (they keep their subprocess until idle eviction
    /// or explicit cancel).
    pub async fn deregister_spec(&self, id: &str) -> Option<SessionAgentSpec> {
        self.specs.write().await.remove(id)
    }

    /// Spawn a fresh subprocess for the given session. If the session
    /// already has a bound agent, the old one is killed first (idempotent
    /// on retry).
    pub async fn start_for_session(
        self: &Arc<Self>,
        session_id: Uuid,
        agent_id: &str,
    ) -> anyhow::Result<Arc<SessionAgent>> {
        let spec = self
            .specs
            .read()
            .await
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("no session_agent spec for: {agent_id}"))?
            .clone();

        // Honour per-spec soft cap (None = unlimited).
        if let Some(max) = spec.max_sessions {
            let count = self
                .agents
                .lock()
                .await
                .values()
                .filter(|a| a.agent_id == agent_id)
                .count();
            if count >= max {
                anyhow::bail!(
                    "session_agent '{agent_id}' reached max_sessions ({max}); refuse spawn"
                );
            }
        }

        let mut cmd = Command::new(&spec.command);
        cmd.args(&spec.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);
        // Apply per-spec env vars. Empty values are skipped so the
        // subprocess inherits the daemon's env (the daemon's env may
        // be populated after config load by the env_watcher, and a
        // bare `${env:VAR}` expansion can land empty at load time).
        for (k, v) in &spec.env {
            if !v.is_empty() {
                cmd.env(k, v);
            }
        }
        // EP-2026-08-15 (Fix 4): propagate tools_allowlist to the subprocess
        // via env var so the agent binary can intersect the tool list it
        // exposes to the LLM. Comma-separated names; empty/missing = no
        // restriction. We only set the var when the allowlist is non-empty
        // so subprocesses without one don't get an empty string in env.
        if let Some(allow) = &spec.tools_allowlist {
            if !allow.is_empty() {
                cmd.env("NEUROX_TOOLS_ALLOWLIST", allow.join(","));
            }
        }
        let child = cmd.spawn().map_err(|e| {
            anyhow::anyhow!("failed to spawn '{cmd}' for session {session_id}: {e}", cmd = spec.command)
        })?;
        let pid = child.id();
        let protocol: SharedProtocol = Arc::new(JsonRpcStdio::new(child));

        let agent = Arc::new(SessionAgent {
            session_id,
            agent_id: agent_id.to_string(),
            pid,
            protocol,
            started_at: Instant::now(),
            last_active: Arc::new(parking_lot::Mutex::new(Instant::now())),
            spec,
            inflight: Arc::new(AtomicU64::new(0)),
            // Fresh subprocess → needs history seed on first dispatch.
            history_seeded: Arc::new(AtomicBool::new(false)),
        });

        let mut agents = self.agents.lock().await;
        if let Some(old) = agents.insert(session_id, agent.clone()) {
            // Defensive: session already had a process running. Kill it.
            warn!(
                session_id = %session_id,
                agent_id = %old.agent_id,
                "session already had a process — replacing"
            );
            let _ = old.protocol.shutdown().await;
        }

        info!(
            session_id = %session_id,
            agent_id = %agent_id,
            pid = ?pid,
            "session agent spawned"
        );
        Ok(agent)
    }

    /// Look up the live agent for a session.
    pub async fn get(self: &Arc<Self>, session_id: Uuid) -> Option<Arc<SessionAgent>> {
        self.agents.lock().await.get(&session_id).cloned()
    }

    /// EP-2026-08-31: validate that the cached agent's subprocess is
    /// actually alive. The daemon's map can hold an `Arc<SessionAgent>`
    /// whose subprocess died (OOM, crash, manual kill, idle eviction
    /// race). When the daemon hands out a dead handle, the next
    /// `protocol.stream()` call hangs on `read_line` because the
    /// pipe never produces data — the user sees a 30s timeout per
    /// request. This helper does a non-destructive `kill(pid, 0)`
    /// probe. `true` if the agent is gone OR the cached pid is
    /// absent / dead.
    pub fn is_subprocess_alive(agent: &SessionAgent) -> bool {
        match agent.pid {
            None => false,
            Some(pid) => unsafe { probe_pid(pid) },
        }
    }

    /// EP-2026-08-31: `get_or_respawn` returns the cached agent only
    /// if its subprocess is still alive. Otherwise removes the dead
    /// entry from the map, spawns a fresh subprocess with the same
    /// `agent_id`, and returns the new agent. Returns `None` if
    /// either no entry exists or respawn fails.
    pub async fn get_or_respawn(
        self: &Arc<Self>,
        session_id: Uuid,
    ) -> Option<Arc<SessionAgent>> {
        // Take a quick snapshot first to release the lock before any
        // (potentially blocking) spawn. If the cached agent's
        // subprocess is alive, return it without touching the map.
        let needs_respawn = {
            let agents = self.agents.lock().await;
            match agents.get(&session_id) {
                Some(a) => !Self::is_subprocess_alive(a),
                None => return None,
            }
        };
        if !needs_respawn {
            return self.agents.lock().await.get(&session_id).cloned();
        }
        // Subprocess is dead. Remove the stale entry and respawn.
        let agent_id = {
            let mut agents = self.agents.lock().await;
            let old = agents.remove(&session_id);
            old.map(|a| a.agent_id.clone())
        };
        let Some(agent_id) = agent_id else {
            return None;
        };
        tracing::warn!(
            session_id = %session_id,
            agent_id = %agent_id,
            "session agent subprocess died — respawning"
        );
        match self.start_for_session(session_id, &agent_id).await {
            Ok(a) => Some(a),
            Err(e) => {
                tracing::error!(session_id = %session_id, error = %e, "respawn failed");
                None
            }
        }
    }

    /// Drop any session agent whose subprocess has died. Called by
    /// the daemon's idle-eviction sweep so dead-but-cached entries
    /// don't accumulate forever. Returns the number of entries
    /// dropped.
    pub async fn reap_dead(&self) -> usize {
        // 1. collect dead (sid, agent_id) pairs.
        let dead: Vec<(Uuid, String)> = {
            let agents = self.agents.lock().await;
            agents
                .iter()
                .filter(|(_, a)| !Self::is_subprocess_alive(a))
                .map(|(sid, a)| (*sid, a.agent_id.clone()))
                .collect()
        };
        let n = dead.len();
        if n == 0 {
            return 0;
        }
        // 2. remove from map.
        {
            let mut agents = self.agents.lock().await;
            for (sid, _) in &dead {
                agents.remove(sid);
            }
        }
        for (sid, agent_id) in dead {
            tracing::warn!(
                session_id = %sid,
                agent_id = %agent_id,
                "reaped dead session agent"
            );
        }
        n
    }

    /// Update `last_active`. Called on every dispatch.
    pub async fn touch(self: &Arc<Self>, session_id: Uuid) {
        if let Some(agent) = self.agents.lock().await.get(&session_id) {
            agent.touch();
        }
    }

    /// Stop and remove the session's subprocess. Idempotent on already-gone.
    pub async fn stop(self: &Arc<Self>, session_id: Uuid) -> anyhow::Result<()> {
        let agent = self.agents.lock().await.remove(&session_id);
        match agent {
            Some(agent) => {
                let _ = agent.protocol.shutdown().await;
                info!(
                    session_id = %session_id,
                    agent_id = %agent.agent_id,
                    pid = ?agent.pid,
                    "session agent stopped"
                );
                Ok(())
            }
            None => {
                // Tolerate double-stop: the second call is a no-op.
                Ok(())
            }
        }
    }

    /// Snapshot of all live session agents.
    pub async fn list(self: &Arc<Self>) -> Vec<SessionAgentInfo> {
        let agents = self.agents.lock().await;
        agents
            .values()
            .map(|a| SessionAgentInfo {
                session_id: a.session_id,
                agent_id: a.agent_id.clone(),
                pid: a.pid,
                command: a.spec.command.clone(),
                args: a.spec.args.clone(),
                uptime_secs: a.started_at.elapsed().as_secs(),
                idle_secs: a.last_active.lock().elapsed().as_secs(),
                max_idle_secs: a.spec.idle_timeout_secs,
                inflight: a.inflight.load(std::sync::atomic::Ordering::Relaxed),
            })
            .collect()
    }

    /// Kill every live subprocess. Called on daemon shutdown.
    pub async fn stop_all(self: &Arc<Self>) {
        let mut agents = self.agents.lock().await;
        let ids: Vec<Uuid> = agents.keys().copied().collect();
        for id in ids {
            if let Some(agent) = agents.remove(&id) {
                let _ = agent.protocol.shutdown().await;
            }
        }
        info!(count = agents.len(), "session agents cleared");
    }

    /// Sweep idle sessions, kill the ones whose `idle_secs` exceed their
    /// spec's `idle_timeout_secs`. Also reaps dead subprocesses
    /// (manual kill / crash / OOM). Returns the evicted session ids.
    pub async fn evict_idle(self: &Arc<Self>) -> Vec<Uuid> {
        let mut agents = self.agents.lock().await;
        let now = Instant::now();
        let to_remove: Vec<Uuid> = agents
            .iter()
            .filter_map(|(sid, a)| {
                let idle = now.duration_since(*a.last_active.lock());
                let timed_out = idle >= Duration::from_secs(a.spec.idle_timeout_secs);
                let dead = !Self::is_subprocess_alive(a);
                if timed_out || dead {
                    Some(*sid)
                } else {
                    None
                }
            })
            .collect();
        for id in &to_remove {
            if let Some(agent) = agents.remove(id) {
                let _ = agent.protocol.shutdown().await;
            }
        }
        if !to_remove.is_empty() {
            info!(count = to_remove.len(), "idle/dead session agents evicted");
        }
        to_remove
    }

    /// Total number of running session agents.
    pub async fn running_count(self: &Arc<Self>) -> usize {
        self.agents.lock().await.len()
    }
}

impl Default for SessionAgentPool {
    fn default() -> Self {
        Self::new(HashMap::new())
    }
}

// ─── Persistent agent subprocesses (EP-0004 wave 3) ───────────────
//
// EP-2026-08-15 added `core::agent_runtime` for spawning one subprocess
// per agent_id at daemon startup, with JSON-RPC-over-stdio dispatch.
// EP-0004 wave 3 folds that module into SessionAgentPool: the
// persistent-agent state lives next to the per-session state in one
// place. The wire protocol (raw stdio JSON-RPC) is preserved — only
// the owning type changes.

/// Live handle for a persistent agent subprocess. The Child is wrapped
/// in a Mutex so we can health-check it concurrently with chat dispatch.
/// `stdin` and `stdout` are extracted once at spawn time and held in Arcs
/// so chat dispatch can write/read without re-entering the Child handle.
pub struct PersistentAgentHandle {
    pub spec: InProcessAgentSpec,
    pub child: Arc<Mutex<Child>>,
    pub stdin: Arc<Mutex<Option<ChildStdin>>>,
    pub stdout: Arc<Mutex<Option<ChildStdout>>>,
    pub stderr: Option<ChildStderr>,
    pub started_at: std::time::Instant,
    /// JSON-RPC id counter — guarantees unique IDs per request, so the
    /// subprocess can correlate responses back to the request.
    pub next_id: Arc<AtomicU64>,
}

/// Per-subprocess stdio lock. Two concurrent calls to the same agent
/// must serialize so requests don't interleave on stdio.
#[derive(Default)]
pub struct PersistentIoLocks {
    stdin: HashMap<String, Arc<Mutex<()>>>,
}

/// Subset of an agent's response we care about.
#[derive(Debug, Clone)]
pub struct PersistentChatOutcome {
    pub text: String,
}

impl SessionAgentPool {
    /// Spawn one persistent agent subprocess for the given spec.
    pub async fn spawn_persistent(
        &self,
        spec: InProcessAgentSpec,
        binary_path: &PathBuf,
    ) -> anyhow::Result<()> {
        let mut g = self.persistent.write().await;
        if g.contains_key(&spec.id) {
            anyhow::bail!("agent '{}' already running", spec.id);
        }

        let identity_dir = spec
            .resolved_identity_dir()
            .to_string_lossy()
            .into_owned();

        info!(
            "[session-agents] spawning persistent agent id={} identity_dir={} binary={}",
            spec.id,
            identity_dir,
            binary_path.display()
        );

        let mut cmd = Command::new(binary_path);
        cmd.arg("--id").arg(&spec.id);
        cmd.arg("--identity-dir").arg(&identity_dir);
        cmd.envs(std::env::vars());
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit());
        cmd.kill_on_drop(false);

        let mut child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);
        info!(
            "[session-agents] persistent agent id={} started pid={}",
            spec.id, pid
        );

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        g.insert(
            spec.id.clone(),
            PersistentAgentHandle {
                spec,
                child: Arc::new(Mutex::new(child)),
                stdin: Arc::new(Mutex::new(stdin)),
                stdout: Arc::new(Mutex::new(stdout)),
                stderr,
                started_at: std::time::Instant::now(),
                next_id: Arc::new(AtomicU64::new(1)),
            },
        );
        Ok(())
    }

    /// Spawn N persistent agents from the config spec list. Failures
    /// are logged and skipped (not propagated) — the daemon must keep
    /// running even if one agent fails to start.
    pub async fn spawn_persistent_from_config(
        &self,
        specs: Vec<InProcessAgentSpec>,
        binary_path: &PathBuf,
    ) -> anyhow::Result<()> {
        for spec in specs {
            if let Err(e) = self.spawn_persistent(spec, binary_path).await {
                warn!("[session-agents] failed to spawn persistent agent: {e}");
            }
        }
        Ok(())
    }

    /// Sends a single "process" JSON-RPC request to the named
    /// persistent subprocess's stdin and reads the matching response
    /// line from stdout. Locks per subprocess so two concurrent calls
    /// to the SAME agent serialize naturally (avoids stdio interleaving).
    pub async fn send_chat_persistent(
        &self,
        agent_id: &str,
        text: &str,
        session_id: Uuid,
        tools: serde_json::Value,
    ) -> anyhow::Result<PersistentChatOutcome> {
        // Clone the IO handles cheaply so we don't hold the global
        // lock while waiting on I/O.
        let (stdin_arc, stdout_arc, next_id) = {
            let g = self.persistent.read().await;
            let h = g
                .get(agent_id)
                .ok_or_else(|| anyhow::anyhow!("agent '{agent_id}' not running"))?;
            (h.stdin.clone(), h.stdout.clone(), h.next_id.clone())
        };

        // Per-agent lock so concurrent calls to the SAME agent don't
        // interleave on stdio. Different agents can run in parallel.
        let lock = {
            let mut locks = self.persistent_locks.lock().await;
            locks
                .stdin
                .entry(agent_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard: OwnedMutexGuard<_> = lock.lock_owned().await;

        // Hold the IO guards for the duration of the stdio use via
        // `as_mut()` — the borrow keeps the guard alive implicitly.
        let mut stdin_guard = stdin_arc.lock().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent '{agent_id}' stdin already closed"))?;

        let mut stdout_guard = stdout_arc.lock().await;
        let stdout = stdout_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("agent '{agent_id}' stdout already closed"))?;

        let req_id = next_id.fetch_add(1, Ordering::SeqCst);
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "process",
            "params": {
                "params": {
                    "text": text,
                    "tools": tools,
                },
                "session_id": session_id.to_string(),
            }
        });
        let req_line = format!("{}\n", req.to_string());

        let writer = stdin;
        writer
            .write_all(req_line.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("write to agent stdin failed: {e}"))?;
        writer
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("flush to agent stdin failed: {e}"))?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let read_fut = reader.read_line(&mut line);
        let line = match tokio::time::timeout(Duration::from_secs(120), read_fut).await {
            Ok(Ok(n)) if n > 0 => line,
            Ok(Ok(_)) => anyhow::bail!("agent stdout closed before reply"),
            Ok(Err(e)) => anyhow::bail!("read agent stdout: {e}"),
            Err(_) => anyhow::bail!("agent '{agent_id}' timed out after 120s"),
        };
        drop(reader);

        let v: serde_json::Value = serde_json::from_str(line.trim())?;
        if let Some(err) = v.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown")
                .to_string();
            anyhow::bail!("agent reported error: {msg}");
        }
        let result = v
            .get("result")
            .ok_or_else(|| anyhow::anyhow!("response missing 'result' field"))?;
        let text = result
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        Ok(PersistentChatOutcome { text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// EP-2026-08-31: probe_pid must NOT return `true` for a pid
    /// whose process is gone. The previous libc::kill binding
    /// silently shadowed and always returned 0, which let dead
    /// agents linger in the map and hang the next dispatch for
    /// 30s. Regressing this re-introduces the original bug.
    #[test]
    fn probe_pid_returns_false_for_dead_process() {
        // Spawn /bin/true, capture its pid, wait for it to exit.
        let child = std::process::Command::new("/bin/true")
            .spawn()
            .expect("spawn /bin/true");
        let pid = child.id();
        // Give the kernel time to reap the zombie. On Linux the
        // process is gone from the process table as soon as it
        // exits, but we wait a moment to be safe.
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            !unsafe { probe_pid(pid) },
            "probe_pid({}) should return false after child exited",
            pid
        );
    }

    #[test]
    fn probe_pid_returns_true_for_self() {
        let pid = std::process::id();
        assert!(unsafe { probe_pid(pid) }, "self pid must probe alive");
    }
}
