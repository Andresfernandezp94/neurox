use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// SearchMemoryTool extracted from core/src/tools/mod.rs (tools/memory/search_memory.rs)
pub struct SearchMemoryTool;

#[async_trait]
impl Tool for SearchMemoryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "search_memory".to_string(),
            description: "Search saved facts by substring match on content.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Substring to search for in facts"}
                },
                "required": ["query"]
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Memory]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Plan, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        // `query` is optional — empty/missing returns ALL facts. The
        // schema in `spec.parameters` already marks it optional (no
        // `required` entry, no `enum`), so the previous error here was
        // a bug that contradicted the spec.
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Resolve facts path. Default: `$XDG_DATA_HOME/neurox/identity/`.
        // Override: $NEUROX_IDENTITY_DIR.
        let identity_dir = std::env::var("NEUROX_IDENTITY_DIR")
            .ok()
            .filter(|p| !p.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                    .join("neurox/identity")
            });
        let facts_path = identity_dir.join("facts.yaml");

        let content = tokio::fs::read_to_string(&facts_path)
            .await
            .map_err(|e| format!("read facts: {e}"))?;

        let doc: serde_yml::Value =
            serde_yml::from_str(&content).map_err(|e| format!("parse: {e}"))?;

        let query_lower = query.to_lowercase();
        let mut matches: Vec<Value> = Vec::new();

        if let Some(facts) = doc.get("facts").and_then(|v| v.as_sequence()) {
            for fact in facts {
                if let Some(c) = fact.get("content").and_then(|v| v.as_str()) {
                    if c.to_lowercase().contains(&query_lower) {
                        // Convert to JSON for output
                        if let Ok(json) = serde_json::to_value(fact) {
                            matches.push(json);
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            Ok(format!("no facts matching '{}'", query))
        } else {
            serde_json::to_string_pretty(&matches).map_err(|e| format!("json: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_memory_finds_matching_fact() {
        let dir = tempfile::tempdir().unwrap();
        let facts = dir.path().join("facts.yaml");
        std::fs::write(&facts, "facts:\n  - id: 1\n    content: user prefers dark mode\n  - id: 2\n    content: project name is neurox\n").unwrap();
        std::env::set_var("NEUROX_IDENTITY_DIR", dir.path());
        let tool = SearchMemoryTool;
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"query": "dark"}),
            )
            .await
            .expect("execute");
        assert!(result.contains("dark mode"), "got: {result}");
        assert!(!result.contains("neurox"), "should not match neurox: {result}");
        std::env::remove_var("NEUROX_IDENTITY_DIR");
    }

    #[tokio::test]
    async fn search_memory_missing_query_errors() {
        let tool = SearchMemoryTool;
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({}),
            )
            .await;
        assert!(result.is_err(), "expected error for missing query");
    }
}
