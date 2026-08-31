use serde_json::Value;
use async_trait::async_trait;
use tokio::process::Command;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// ClipboardReadTool extracted from core/src/tools/mod.rs (tools/desktop/clipboard_read.rs)
pub struct ClipboardReadTool;

#[async_trait]
impl Tool for ClipboardReadTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "clipboard_read".to_string(),
            description: "Read current clipboard content (Wayland wl-paste).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Desktop]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, _args: Value) -> Result<String, String> {
        let output = Command::new("wl-paste")
            .output()
            .await
            .map_err(|e| format!("wl-paste failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("wl-paste error: {}", stderr));
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(text)
    }
}
