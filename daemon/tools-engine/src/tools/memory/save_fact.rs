use serde_json::Value;
use async_trait::async_trait;
use crate::tools::atomic_store::mutate_store;
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
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "missing 'content' (or empty)".to_string())?
            .to_string();
        let fact_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("learned")
            .to_string();

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

        // Generate ID + timestamp up front (deterministic per call).
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let content_for_closure = content.clone();
        let id_for_closure = id.clone();

        // SR-fix: use the shared `mutate_store` helper which holds an
        // in-process Mutex for the read-modify-write AND writes
        // atomically (tmpfile + rename). Without this, concurrent
        // save_fact calls would race, produce partial YAML, and the
        // second parser would fail with "could not find expected :".
        mutate_store(facts_path.clone(), move |existing| {
            let mut doc: serde_yml::Value = if existing.is_empty() {
                serde_yml::from_str("facts: []")
                    .map_err(|e| format!("init yaml: {e}"))?
            } else {
                serde_yml::from_str(&existing)
                    .map_err(|e| format!("parse yaml: {e}"))?
            };

            let new_fact = serde_yml::to_value(serde_json::json!({
                "id": id_for_closure,
                "type": fact_type,
                "content": content_for_closure,
                "source": "agent",
                "created": now,
                "last_confirmed": now,
                "confidence": 0.9,
                "active": true
            }))
            .map_err(|e| format!("serialize: {e}"))?;

            if let Some(facts) = doc.get_mut("facts").and_then(|v| v.as_sequence_mut()) {
                facts.push(new_fact);
            } else {
                return Err("facts.yaml has invalid structure (missing 'facts' array)".to_string());
            }

            let yaml_str = serde_yml::to_string(&doc)
                .map_err(|e| format!("serialize yaml: {e}"))?;
            Ok(((), yaml_str))
        })
        .await?;

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

    // SR-fix: concurrent save_fact calls must all succeed (no YAML
    // corruption, no lost updates).
    #[tokio::test]
    async fn save_fact_concurrent_does_not_corrupt_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("NEUROX_IDENTITY_DIR", dir.path());

        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push(tokio::spawn(async move {
                let tool = SaveFactTool;
                tool.execute(
                    &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                    serde_json::json!({"content": format!("concurrent fact {i}")}),
                )
                .await
            }));
        }
        for h in handles {
            assert!(h.await.unwrap().is_ok(), "all 10 saves should succeed");
        }
        std::env::remove_var("NEUROX_IDENTITY_DIR");

        // Verify the YAML is well-formed and has 10 facts.
        let yaml = std::fs::read_to_string(dir.path().join("facts.yaml")).unwrap();
        let doc: serde_yml::Value = serde_yml::from_str(&yaml).unwrap();
        let count = doc.get("facts").and_then(|v| v.as_sequence()).unwrap().len();
        assert_eq!(count, 10, "expected 10 facts, got {count}");
    }
}
