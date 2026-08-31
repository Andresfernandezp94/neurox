use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::tools::{ToolCategory, Mode};
// SaveFactTool extracted from core/src/tools/mod.rs (tools/memory/save_fact.rs)
pub struct SaveFactTool;

#[async_trait]
impl Tool for SaveFactTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "save_fact".to_string(),
            description: "Save a learned fact to persistent memory (identity_dir/facts.yaml; see NEUROX_IDENTITY_DIR).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "The fact to remember"},
                    "type": {"type": "string", "default": "learned", "description": "Fact type (learned, preference, context)"}
                },
                "required": ["content"]
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Memory]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Chat]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'content'".to_string())?;
        let fact_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("learned");

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

        // Ensure directory exists
        if let Some(parent) = facts_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("mkdir: {e}"))?;
        }

        // Read existing facts
        let existing = tokio::fs::read_to_string(&facts_path)
            .await
            .unwrap_or_default();
        let mut doc: serde_yml::Value = if existing.is_empty() {
            serde_yml::from_str("facts: []").unwrap()
        } else {
            serde_yml::from_str(&existing).map_err(|e| format!("parse yaml: {e}"))?
        };

        // Generate ID
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Build new fact
        let new_fact = serde_yml::to_value(serde_json::json!({
            "id": id,
            "type": fact_type,
            "content": content,
            "source": "agent",
            "created": now,
            "last_confirmed": now,
            "confidence": 0.9,
            "active": true
        }))
        .map_err(|e| format!("serialize: {e}"))?;

        // Append to facts array
        if let Some(facts) = doc.get_mut("facts").and_then(|v| v.as_sequence_mut()) {
            facts.push(new_fact);
        } else {
            return Err("facts.yaml has invalid structure (missing 'facts' array)".to_string());
        }

        // Write back
        let yaml_str = serde_yml::to_string(&doc).map_err(|e| format!("serialize: {e}"))?;
        tokio::fs::write(&facts_path, &yaml_str)
            .await
            .map_err(|e| format!("write: {e}"))?;

        Ok(format!("saved fact '{}' (id: {})", content, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_fact_writes_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let tool = SaveFactTool;
        // Override identity_dir via env var
        std::env::set_var("NEUROX_IDENTITY_DIR", dir.path());
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"content": "user prefers dark mode"}),
            )
            .await
            .expect("execute");
        assert!(result.contains("user prefers dark mode"));
        std::env::remove_var("NEUROX_IDENTITY_DIR");
    }

    #[tokio::test]
    async fn save_fact_missing_content_errors() {
        let tool = SaveFactTool;
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({}),
            )
            .await;
        assert!(result.is_err(), "expected error for missing content");
    }
}
