// TodoAddTool — extracted from sidebar's TodoAddTool.qml to the daemon.
// Persists a new task in the JSON-backed todo store.

use async_trait::async_trait;
use serde_json::{json, Value};
use chrono::Utc;

use crate::tools::task_management::todo_store::{read_todos, write_todos, Todo, format};
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};

pub struct TodoAddTool;

#[async_trait]
impl Tool for TodoAddTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todo_add".to_string(),
            description: "Add a new task to the user's todo list. Todos persist across daemon restarts and are visible to all future conversations.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "The task description." },
                    "priority": { "type": "string", "enum": ["low", "normal", "high"], "default": "normal",
                                   "description": "Task priority (default: normal)." }
                },
                "required": ["content"]
            }),
            requires_approval: false,
            ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::TaskManagement]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Plan, Mode::Build]
    }

    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "missing 'content' (or empty)".to_string())?;
        let priority = args
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("normal")
            .to_string();

        let mut todos = read_todos().await;
        let id = format!("t{}", uuid::Uuid::new_v4().simple().to_string()[..8].to_string());
        let todo = Todo {
            id: id.clone(),
            content: content.trim().to_string(),
            priority: priority.clone(),
            status: "pending".to_string(),
            created_at: Utc::now().timestamp_millis(),
            done_at: None,
        };
        let formatted = format(&todo);
        todos.push(todo);
        write_todos(&todos).await?;
        Ok(format!("added todo: {}", formatted))
    }
}
