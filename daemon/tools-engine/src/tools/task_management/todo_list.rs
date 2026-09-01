// TodoListTool — list all todos (optionally filtered by status).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::task_management::todo_store::{read_todos, format};
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};

pub struct TodoListTool;

#[async_trait]
impl Tool for TodoListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todo_list".to_string(),
            description: "List the user's current todos. Optionally filter by status (pending, done, all).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["all", "pending", "done"], "default": "all",
                                "description": "Filter by todo status (default: all)." }
                }
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
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let todos = read_todos().await;
        let filtered: Vec<_> = if status == "all" {
            todos.iter().collect()
        } else {
            todos.iter().filter(|t| t.status == status).collect()
        };

        if filtered.is_empty() {
            Ok(format!("no todos (filter: {status})"))
        } else {
            let body = filtered.iter().map(|t| format(t)).collect::<Vec<_>>().join("\n");
            Ok(format!("{} todo(s) [{}]:\n{}", filtered.len(), status, body))
        }
    }
}
