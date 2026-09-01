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
// Same pattern as save_fact: every operation reads, mutates, writes the
// file. No in-process cache — the file is the single source of truth.
// This is fine for a todo list (low write rate, single user).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Write the full list back to disk. Caller is responsible for
/// reading + mutating + writing.
pub async fn write_todos(items: &[Todo]) -> Result<(), String> {
    let path = todo_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(items)
        .map_err(|e| format!("serialize: {e}"))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("write: {e}"))?;
    Ok(())
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
