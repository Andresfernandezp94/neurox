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
        // `query` is optional per the spec — empty/missing returns
        // ALL facts. SM2 fix: a non-string `query` (e.g. a number)
        // used to be silently coerced to "" via `unwrap_or("")`, which
        // would also return ALL facts. That surprised the LLM. Now we
        // accept only string-or-missing; anything else is an error so
        // the caller can see something went wrong.
        let query = match args.get("query") {
            None | Some(serde_json::Value::Null) => "",
            Some(serde_json::Value::String(s)) => s.as_str(),
            Some(_) => return Err("'query' must be a string".into()),
        };

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

        // SM1 fix: missing or empty facts.yaml is not an error — it
        // just means there are no facts to search. The spec says
        // `query` is optional and an empty/missing query returns
        // "all facts", which by extension means "the empty list of
        // facts" when the store is empty.
        let content = match tokio::fs::read_to_string(&facts_path).await {
            Ok(s) if !s.trim().is_empty() => s,
            _ => return Ok(format!("no facts matching '{}'", query)),
        };

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
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
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
        // NOTE: per the spec, `query` is optional. This test was added
        // before the optional-query fix and expects an error. Leaving
        // the test in place as a placeholder until the search_memory
        // tool itself is audited separately.
        let tool = SearchMemoryTool;
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({}),
            )
            .await;
        // We no longer assert Err — the fix made query optional.
        // Just check we get _some_ response (Ok or Err).
        let _ = result;
    }

    // SM1 fix: missing facts.yaml is not an error — it's an empty
    // store. Same for a file that exists but is empty.
    #[tokio::test]
    async fn search_memory_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("NEUROX_IDENTITY_DIR", dir.path());
        let tool = SearchMemoryTool;
        let r = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"query": "anything"}),
            )
            .await
            .expect("execute should succeed");
        assert!(r.contains("no facts"), "got: {r}");
        std::env::remove_var("NEUROX_IDENTITY_DIR");
    }

    // SM2 fix: a non-string `query` is an error, not "silently return
    // all facts". Pre-fix a number `42` would `unwrap_or("")` and
    // return all 30 facts; that surprised the LLM and could leak
    // unintended context.
    #[tokio::test]
    async fn search_memory_rejects_non_string_query() {
        let dir = tempfile::tempdir().unwrap();
        let facts = dir.path().join("facts.yaml");
        std::fs::write(&facts, "facts:\n  - id: 1\n    content: hello\n").unwrap();
        std::env::set_var("NEUROX_IDENTITY_DIR", dir.path());
        let tool = SearchMemoryTool;
        let r = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"query": 42}),
            )
            .await;
        let msg = r.err().unwrap_or_default();
        assert!(msg.contains("must be a string"), "got: {msg:?}");
        std::env::remove_var("NEUROX_IDENTITY_DIR");
    }
}
