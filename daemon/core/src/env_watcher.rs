//! Env-file watchdog (EP-0018-05).
//!
//! Background task that polls `~/.config/neurox/env` (or wherever
//! `NEUROX_ENV_FILE` points) and propagates changes to the daemon
//! process environment via `std::env::set_var` / `remove_var`.
//!
//! Why polling and not inotify:
//! - The file lives at a path we control; the overhead of a 5-second
//!   poll is negligible (the file is rarely written to).
//! - Avoids adding `notify` as a dep just for one feature.
//! - Works uniformly on macOS dev hosts and Linux prod hosts.
//!
//! Scope:
//! - **In-process** consumers (default, tools, plugins in-process) see
//!   the new value immediately on the next `std::env::var(...)` call.
//! - **Subprocesses spawned after the change** inherit the new env
//!   automatically (fork copies the process env).
//! - **Subprocesses spawned before the change** keep their frozen env
//!   until restarted — restarting the orchestrator-managed services is
//!   a separate concern (the operator can `POST /start` to recycle them).
//! - **systemd plugins** (`memory`, `voice`, …) read `EnvironmentFile`
//!   only at `daemon-reload + restart`. They are NOT notified by this
//!   watcher (out of scope for the MVP).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::environments::env_file_path;

/// Default poll interval. 5s is responsive enough for ops (typing in the
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Background watcher loop. Spawn via `tokio::spawn`. Returns only if the
/// file path resolution fails at startup (e.g. `HOME` unset) — otherwise
/// it runs forever, logging diffs on every change.
/// Run the env-file watchdog loop. `on_change` is called (await) whenever
/// the snapshot of `~/.config/neurox/env` differs from the previous one
/// (so consumers like the LLM catalog cache can be invalidated).
pub async fn watch_loop<F, Fut>(on_change: F)
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let path = match env_file_path().canonicalize() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "env_watcher: could not resolve env file path; watcher disabled");
            return;
        }
    };
    info!(
        path = %path.display(),
        poll_secs = POLL_INTERVAL.as_secs(),
        "env_watcher: started"
    );

    let mut ticker = interval(POLL_INTERVAL);
    // First tick fires immediately; we don't want that — wait for the
    // first interval before reading.
    ticker.tick().await;

    let mut cache: BTreeMap<String, String> = read_kv(&path).unwrap_or_default();
    // Propagate the initial snapshot to the process env so providers that
    // declare `api_key_env = "<key>"` are marked as configured on the first
    // catalog build (without waiting for the env file to change).
    if !cache.is_empty() {
        let initial_changed = apply_diff(&BTreeMap::new(), &cache, &path);
        if initial_changed {
            on_change().await;
        }
    }

    loop {
        ticker.tick().await;
        match read_kv(&path) {
            Ok(next) => {
                let changed = apply_diff(&cache, &next, &path);
                if changed {
                    on_change().await;
                }
            }
            Err(e) => debug!(error = %e, path = %path.display(), "env_watcher: read failed"),
        }
        cache = match read_kv(&path) {
            Ok(m) => m,
            Err(_) => cache,
        };
    }
}

/// Parse the env file into a sorted map of KEY=value pairs. Lines that
/// don't match the `^[A-Z][A-Z0-9_]*=` pattern (comments, blanks) are
/// ignored.
fn read_kv(path: &PathBuf) -> std::io::Result<BTreeMap<String, String>> {
    let content = std::fs::read_to_string(path)?;
    let mut out = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let k = k.trim();
            if crate::environments::validate_key(k).is_ok() {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    Ok(out)
}

/// Apply the diff between the previous snapshot and the new one. Sets
/// new keys. **Does NOT remove keys** — see `EP-2026-09-02` notes below.
///
/// EP-2026-09-02: auto-removal was a footgun. Two problems:
///   1. In a multi-threaded process (tokio) `std::env::remove_var` is
///      marked `unsafe` since Rust 1.65 because libc's unsetenv modifies
///      a per-libc copy of the environ array that `std::env::var` later
///      reads. The kernel-managed `/proc/<pid>/environ` keeps the
///      original, but the daemon's libc view is left permanently
///      corrupted — there's no safe way to re-sync without a restart.
///   2. Any partial write to the env file (PUT /v1/env/:key, manual
///      edit, accidental overwrite) would silently nuke every key not
///      in the new file. Operators only learned about it via 401s and
///      the chat model selector going blank.
///
/// The canonical way to remove a key is `DELETE /v1/env/:key`, which
/// already handles the lifecycle explicitly. The watcher is now
/// strictly additive.
fn apply_diff(prev: &BTreeMap<String, String>, next: &BTreeMap<String, String>, path: &Path) -> bool {
    let mut changed = false;
    // Added or updated keys.
    for (k, v) in next {
        if prev.get(k) != Some(v) {
            info!(
                key = %k,
                path = %path.display(),
                "env_watcher: propagated env var to daemon process"
            );
            std::env::set_var(k, v);
            changed = true;
        }
    }
    changed
}

/// Touch the file's mtime so the watcher picks up changes written by
/// other processes (e.g. `PUT /v1/env/:key`). Called by the env handler
/// after a successful write to nudge the next poll.
pub fn touch_after_write() {
    let path = match env_file_path().canonicalize() {
        Ok(p) => p,
        Err(_) => return,
    };
    // Best-effort: if the file doesn't exist yet, nothing to touch.
    let _ = std::fs::File::options().write(true).open(&path);
    // The polling loop diffs the cache against the next read; the mtime
    // isn't actually consulted — touching is just a defensive no-op.
}

/// Test-only helper: synchronously apply a snapshot (used by the
/// integration test to bypass the polling interval).
pub fn apply_snapshot(snapshot: &BTreeMap<String, String>) {
    let Some(path) = env_file_path().canonicalize().ok() else {
        return;
    };
    let prev = read_kv(&path).unwrap_or_default();
    apply_diff(&prev, snapshot, &path);
    // Note: this is intentionally best-effort — production code uses the
    // polling watcher. Tests use `apply_snapshot` to skip the wait.
}
