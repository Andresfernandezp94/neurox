// TodoRemoveTool — permanently delete a todo by id (full or prefix).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::task_management::todo_store::mutate_todos;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};

pub struct TodoRemoveTool;

#[async_trait]
impl Tool for TodoRemoveTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todo_remove".to_string(),
            description: "Remove a todo from the list permanently. Pass the id (full or prefix, from todo_list output).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "The todo id (full or prefix, from todo_list output)." }
                },
                "required": ["id"]
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
        let needle = args
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "missing 'id'".to_string())?
            .to_string();

        mutate_todos(move |todos| {
            let full_id = todos
                .iter()
                .filter(|t| t.id == needle || t.id.starts_with(&needle))
                .max_by_key(|t| t.id.len())
                .map(|t| t.id.clone());

            let Some(full_id) = full_id else {
                return Err(format!("no todo found with id '{needle}'"));
            };

            let before = todos.len();
            todos.retain(|t| t.id != full_id);
            let removed = before - todos.len();
            Ok(format!("removed {removed} todo(s) ({full_id}). {} remaining.", todos.len()))
        })
        .await
    }
}
