// TodoDoneTool — mark a todo as completed. Accepts the full id or a
// prefix (first 6+ chars) to match what `todo_list` shows the LLM.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};

use crate::tools::task_management::todo_store::{mutate_todos, format as format_todo};
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};

pub struct TodoDoneTool;

#[async_trait]
impl Tool for TodoDoneTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "todo_done".to_string(),
            description: "Mark a todo as completed. Pass the id from todo_list output (prefix match supported).".to_string(),
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

        // mutate_todos holds the exclusive flock so concurrent done
        // calls on the same id serialize. Idempotent: if the todo is
        // already done, the closure no-ops and returns the existing
        // record so the caller still gets a success message.
        let now = Utc::now().timestamp_millis();
        mutate_todos(move |todos| {
            // Prefix match: longest id that starts with `needle` wins.
            let full_id = todos
                .iter()
                .filter(|t| t.id == needle || t.id.starts_with(&needle))
                .max_by_key(|t| t.id.len())
                .map(|t| t.id.clone());
            let Some(full_id) = full_id else {
                return Err(format!("no todo found with id '{needle}'"));
            };
            for t in todos.iter_mut() {
                if t.id == full_id {
                    if t.status != "done" {
                        t.status = "done".to_string();
                        t.done_at = Some(now);
                    }
                }
            }
            // Surface the updated todo back so the caller can format
            // a friendly success message without re-reading.
            let updated = todos
                .iter()
                .find(|t| t.id == full_id)
                .map(format_todo)
                .unwrap_or_else(|| full_id.clone());
            Ok(format!("marked done: {updated}"))
        })
        .await
    }
}
