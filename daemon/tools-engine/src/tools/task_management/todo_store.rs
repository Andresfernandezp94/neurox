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
// Concurrency: every mutation (add/done/remove/clear) goes through
// the shared `mutate_store` helper which holds an in-process Mutex
// for the duration of the read-modify-write cycle and writes
// atomically (tmpfile + rename). The Mutex serializes the critical
// section; the atomic write prevents readers from seeing a torn
// state.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::tools::atomic_store::mutate_store;

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
    mutate_store(path, move |contents| {
        let mut items: Vec<Todo> = if contents.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&contents)
                .map_err(|e| format!("parse todos.json: {e}"))?
        };
        let result = f(&mut items)?;
        let new_contents = serde_json::to_string_pretty(&items)
            .map_err(|e| format!("serialize todos: {e}"))?;
        Ok((result, new_contents))
    })
    .await
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
