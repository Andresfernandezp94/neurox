// Shared helpers for tools that mutate a persistent on-disk store
// (todo_*, save_fact, …).
//
// The pattern is identical across all of them:
//   1. Acquire an in-process Mutex (serializes concurrent writers
//      inside this daemon).
//   2. Read the current file contents.
//   3. Run a closure that parses, mutates, and serializes the new
//      content. The closure returns `(result, new_content)` where
//      `result` is whatever the tool wants to surface to the LLM
//      and `new_content` is the bytes to write back.
//   4. Atomically write the new content (tmpfile + rename) so a
//      concurrent reader can never see a torn intermediate.
//
// `flock(2)` would also work for the cross-process case but in our
// tests it occasionally allowed the read-modify-write to interleave
// (the lock was acquired but the spawned-blocking read happened
// before the lock was fully effective in the same process). A
// tokio::sync::Mutex is more predictable in async code. The store
// files are tiny and the mutex is held only for the duration of a
// single read-modify-write, so contention is negligible.

use std::path::Path;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use uuid::Uuid;

static STORE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock() -> &'static Mutex<()> {
    STORE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Atomically write `contents` to `path`. Writes to a tmpfile in the
/// same directory and renames into place. `rename(2)` is atomic on
/// POSIX so readers always observe either the old or the new file —
/// never a torn intermediate.
async fn atomic_write(path: &Path, contents: String) -> Result<(), String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // Pre-create the target so a concurrent reader never sees
        // ENOENT during the write window.
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("precreate: {e}"))?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let tmp = parent.join(format!(".tmp.{}", Uuid::new_v4()));
        std::fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("rename: {e}"));
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("write-join: {e}"))??;
    Ok(())
}

/// Read the file at `path`, returning `""` if it is missing or empty.
async fn read_file(path: &Path) -> String {
    match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(_) => String::new(),
    }
}

/// Mutate the persistent store at `path` atomically. See module docs
/// for the full pattern. `f` receives the current file content and
/// returns `(result, new_content)`.
pub async fn mutate_store<F, R, P>(path: P, f: F) -> Result<R, String>
where
    F: FnOnce(String) -> Result<(R, String), String> + Send + 'static,
    R: Send + 'static,
    P: AsRef<Path> + Send + 'static,
{
    let path = path.as_ref().to_path_buf();
    let _guard = lock().lock().await;
    let current = read_file(&path).await;
    let (result, new_content) = f(current)?;
    atomic_write(&path, new_content).await?;
    Ok(result)
}
