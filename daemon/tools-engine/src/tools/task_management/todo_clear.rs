// TodoClearTool — clear all todos, or only completed ones.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::task_management::todo_store::{read_todos, write_todos};
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};

pub struct TodoClearTool;

#[async_trait]
impl Tool for TodoClearTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todo_clear".to_string(),
            description: "Clear all todos, or only the completed ones.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "completed_only": { "type": "boolean", "default": false,
                                        "description": "If true, only remove todos already marked done." }
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
        let completed_only = args
            .get("completed_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut todos = read_todos().await;
        let before = todos.len();
        if completed_only {
            todos.retain(|t| t.status != "done");
        } else {
            todos.clear();
        }
        let removed = before - todos.len();
        write_todos(&todos).await?;
        if completed_only {
            Ok(format!("cleared {} completed todo(s). {} remaining.", removed, todos.len()))
        } else {
            Ok(format!("cleared all {} todo(s).", removed))
        }
    }
}
