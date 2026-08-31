use std::time::Duration;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// WebFetchTool extracted from core/src/tools/mod.rs (tools/read/web_fetch.rs)
pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_fetch".to_string(),
            description: "Fetch a URL and return text content. Modes: 'truncated' (8K, default), 'full' (16K), 'selective' (search for keywords, ±10 lines context).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "HTTP or HTTPS URL"},
                    "mode": {"type": "string", "enum": ["truncated", "full", "selective"], "default": "truncated"},
                    "search_terms": {"type": "string", "description": "Space-separated keywords for selective mode"}
                },
                "required": ["url"]
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
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'url'".to_string())?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("truncated");
        let search_terms = args
            .get("search_terms")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let client = reqwest::Client::builder()
            .user_agent("neurox/0.1")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("client: {e}"))?;
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("http: {e}"))?;
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().await.map_err(|e| format!("read: {e}"))?;
        let text = if ct.contains("html") {
            html_to_text(&body)
        } else {
            body
        };

        match mode {
            "full" => {
                let truncated: String = text.chars().take(16000).collect();
                Ok(truncated)
            }
            "selective" => {
                if search_terms.is_empty() {
                    return Err("'search_terms' required for selective mode".to_string());
                }
                let terms: Vec<&str> = search_terms.split_whitespace().collect();
                let lines: Vec<&str> = text.lines().collect();
                let mut selected_lines: std::collections::BTreeSet<usize> =
                    std::collections::BTreeSet::new();

                for (i, line) in lines.iter().enumerate() {
                    let lower = line.to_lowercase();
                    if terms.iter().any(|t| lower.contains(&t.to_lowercase())) {
                        let start = i.saturating_sub(10);
                        let end = (i + 11).min(lines.len());
                        for idx in start..end {
                            selected_lines.insert(idx);
                        }
                    }
                }

                if selected_lines.is_empty() {
                    return Ok(format!(
                        "No matches for '{}' in page content.",
                        search_terms
                    ));
                }

                let mut out = String::new();
                let mut prev: Option<usize> = None;
                for &idx in &selected_lines {
                    if let Some(p) = prev {
                        if idx > p + 1 {
                            out.push_str("\n...\n\n");
                        }
                    }
                    out.push_str(lines[idx]);
                    out.push('\n');
                    prev = Some(idx);
                }
                // Truncate to 16K
                let result: String = out.chars().take(16000).collect();
                Ok(result)
            }
            _ => {
                // truncated (default): 8K
                let truncated: String = text.chars().take(8000).collect();
                Ok(truncated)
            }
        }
    }
}
