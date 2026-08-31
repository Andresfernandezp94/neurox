use tokio::sync::RwLock;
use std::sync::Arc;
use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// ListDirTool extracted from core/src/tools/mod.rs (tools/read/list_dir.rs)
pub struct ListDirTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

#[async_trait]
impl Tool for ListDirTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "list_dir".to_string(),
            description: "List files and subdirectories in a directory. Returns a JSON array with name, type (file/dir), and size.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute or workspace-relative path", "default": "."},
                    "max_entries": {"type": "integer", "default": 200}
                }
            }),
            requires_approval: false,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Filesystem]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build, Mode::Plan]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max = args
            .get("max_entries")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(200) as usize;

        let resolved = resolve_under_workspace(
            &self.workspace_root,
            path,
            &self.sandbox.read().await.readable_paths_resolved(&self.workspace_root),
            false,
        )
        .map_err(|e| format!("path: {e}"))?;
        let mut entries = tokio::fs::read_dir(&resolved)
            .await
            .map_err(|e| format!("readdir: {e}"))?;
        let mut out: Vec<Value> = Vec::new();
        while let Some(e) = entries
            .next_entry()
            .await
            .map_err(|e| format!("entry: {e}"))?
        {
            if out.len() >= max {
                break;
            }
            let meta = e.metadata().await.ok();
            let entry = serde_json::json!({
                "name": e.file_name().to_string_lossy(),
                "type": if meta.as_ref().is_some_and(std::fs::Metadata::is_dir) { "dir" } else { "file" },
                "size": meta.as_ref().map_or(0, std::fs::Metadata::len),
            });
            out.push(entry);
        }
        serde_json::to_string_pretty(&serde_json::json!({
            "path": path,
            "count": out.len(),
            "entries": out,
        }))
        .map_err(|e| format!("json: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sandbox(readable: Vec<String>) -> Arc<RwLock<Box<dyn SandboxConfig>>> {
        Arc::new(RwLock::new(Box::new(TestSandbox {
            enabled: true,
            readable,
        })))
    }

    struct TestSandbox {
        enabled: bool,
        readable: Vec<String>,
    }
    impl SandboxConfig for TestSandbox {
        fn enabled(&self) -> bool { self.enabled }
        fn is_writable(&self, _: &std::path::Path) -> bool { false }
        fn is_readable(&self, path: &std::path::Path) -> bool {
            self.readable.iter().any(|r| path.starts_with(std::path::PathBuf::from(r)))
        }
    }

    #[tokio::test]
    async fn list_dir_returns_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        std::fs::write(dir.path().join("b.txt"), "y").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ListDirTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"path": dir.path().to_string_lossy().to_string()}),
            )
            .await
            .expect("execute");
        assert!(result.contains("a.txt"), "missing a.txt: {result}");
        assert!(result.contains("b.txt"), "missing b.txt: {result}");
        assert!(result.contains("sub"), "missing sub: {result}");
    }

    #[tokio::test]
    async fn list_dir_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = ListDirTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"path": "/etc"}),
            )
            .await;
        assert!(result.is_err());
    }
}
