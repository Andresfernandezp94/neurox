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
                    "query": {"type": "string", "description": "Search query (1-200 chars, trimmed)"},
                    "max_results": {"type": "integer", "default": 5, "minimum": 1, "maximum": 10,
                                     "description": "How many results to return (default 5, max 10)"}
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
        // WS-fix: ddgr crashes with a confusing "string index out of
        // range" error if we hand it an empty string. Catch that
        // early so the LLM sees a clear "empty query" message and we
        // don't ship a panic message back as an error.
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'query'".to_string())?;
        let query = query.trim();
        if query.is_empty() {
            return Err("web_search: empty query".into());
        }
        // WS-fix: cap query length so a runaway LLM doesn't ship a
        // multi-KB string to ddgr (which then rate-limits us and
        // returns useless results anyway). 200 chars is a generous
        // limit for a search query.
        if query.len() > 200 {
            return Err(format!(
                "web_search: query too long ({} chars, max 200)",
                query.len()
            ));
        }

        // WS-fix: let the LLM request N results (default 5, max 10).
        // 10 is a sensible cap to keep the LLM context from being
        // flooded with search results.
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .clamp(1, 10) as usize;

        let output = Command::new("ddgr")
            .arg("--np")
            .arg("-n")
            .arg(max_results.to_string())
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
