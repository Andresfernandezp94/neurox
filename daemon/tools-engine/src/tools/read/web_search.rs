use serde_json::Value;
use async_trait::async_trait;
use tokio::process::Command;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// WebSearchTool extracted from core/src/tools/mod.rs (tools/read/web_search.rs)
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search".to_string(),
            description: "Search the web using DuckDuckGo. Returns JSON results with title, url, and abstract.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"}
                },
                "required": ["query"]
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Web]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Plan, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'query'".to_string())?;

        let output = Command::new("ddgr")
            .arg("--np")
            .arg("-n")
            .arg("5")
            .arg("--json")
            .arg(query)
            .output()
            .await
            .map_err(|e| format!("ddgr not found or failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ddgr failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }
}
