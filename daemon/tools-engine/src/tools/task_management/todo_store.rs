// Persistent JSON-backed todo store for the AI chat.
//
// Each todo is:
//   {
//     "id": "l3k9...",
//     "content": "Add hover effect",
//     "priority": "low" | "normal" | "high",
//     "status":   "pending" | "done",
//     "createdAt": 1735689600000,
//     "doneAt":    1735689700000 | null
//   }
//
// Stored as a top-level array at the configured path (default
// `$XDG_DATA_HOME/neurox/todos.json`, overridable via NEUROX_TODO_DIR).
//
// Concurrency: every mutation (add/done/remove/clear) holds an
// in-process `Mutex` for the duration of the read-modify-write
// cycle AND writes atomically (tmpfile + rename). The Mutex
// serializes the critical section; the atomic write prevents
// readers from seeing a torn state.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub content: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_status")]
    pub status: String,
    pub created_at: i64,
    #[serde(default)]
    pub done_at: Option<i64>,
}

fn default_priority() -> String { "normal".to_string() }
fn default_status() -> String { "pending".to_string() }

/// Path to the todos file. Override via `NEUROX_TODO_DIR`; defaults to
/// `$XDG_DATA_HOME/neurox/todos.json`.
pub fn todo_path() -> PathBuf {
    std::env::var("NEUROX_TODO_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("neurox/todos.json")
        })
}

/// Process-wide async mutex that serializes all todo mutations.
/// OnceLock makes it lazily initialized on first use.
static TODOS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock() -> &'static Mutex<()> {
    TODOS_LOCK.get_or_init(|| Mutex::new(()))
}

/// Read all todos. Returns an empty list if the file is missing or
/// invalid (e.g. fresh install).
pub async fn read_todos() -> Vec<Todo> {
    let path = todo_path();
    let body = match tokio::fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    if body.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&body).unwrap_or_else(|e| {
        tracing::warn!(path = ?path, error = %e, "todos.json unreadable, starting empty");
        Vec::new()
    })
}

/// Atomically write the todos list to disk. Writes to a tmpfile in
/// the same directory and renames into place. `rename(2)` is atomic
/// on POSIX so readers always observe either the old or the new
/// content — never a torn intermediate.
async fn atomic_write_todos(path: &std::path::Path, json: String) -> Result<(), String> {
    use uuid::Uuid;
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
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let tmp = parent.join(format!(".tmp.{}", Uuid::new_v4()));
        std::fs::write(&tmp, json.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
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

/// Atomically mutate the todos list: take the in-process Mutex,
/// call `f` with the current list, then write the result back. The
/// Mutex is held for the duration of `f` so concurrent mutations
/// serialize cleanly.
pub async fn mutate_todos<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut Vec<Todo>) -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    let path = todo_path();
    let _guard = lock().lock().await;
    let mut items = read_todos().await;
    let result = f(&mut items)?;
    let json = serde_json::to_string_pretty(&items)
        .map_err(|e| format!("serialize: {e}"))?;
    atomic_write_todos(&path, json).await?;
    Ok(result)
}

/// Display a single todo in the same `check #id content (priority)` form
/// the sidebar used, so the LLM's output looks the same regardless of
/// which side stored the list.
pub fn format(t: &Todo) -> String {
    let check = if t.status == "done" { "[x]" } else { "[ ]" };
    let short_id = t.id.chars().take(6).collect::<String>();
    let pri = if t.priority != "normal" {
        format!(" ({})", t.priority)
    } else {
        String::new()
    };
    format!("{} #{} {}{}", check, short_id, t.content, pri)
}
