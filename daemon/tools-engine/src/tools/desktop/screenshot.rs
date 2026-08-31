use serde_json::Value;
use async_trait::async_trait;
use tokio::process::Command;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// ScreenshotTool extracted from core/src/tools/mod.rs (tools/desktop/screenshot.rs)
pub struct ScreenshotTool;

#[async_trait]
impl Tool for ScreenshotTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "screenshot".to_string(),
            description: "Take a screenshot of the current screen (grim). Returns the file path."
                .to_string(),
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
        let path = "/tmp/neurox-screenshot.png";
        let output = Command::new("grim")
            .arg(path)
            .output()
            .await
            .map_err(|e| format!("grim failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("grim error: {}", stderr));
        }

        Ok(format!("screenshot saved to {}", path))
    }
}
