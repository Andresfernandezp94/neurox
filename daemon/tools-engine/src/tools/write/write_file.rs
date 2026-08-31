use tokio::sync::RwLock;
use std::sync::Arc;
use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// WriteFileTool extracted from core/src/tools/mod.rs (tools/write/write_file.rs)
pub struct WriteFileTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

#[async_trait]
impl Tool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".to_string(),
            description: "Write/modify a file. Commands: 'create' (overwrite), 'strReplace' (find/replace text), 'insert' (insert at line).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute or workspace-relative path"},
                    "command": {"type": "string", "enum": ["create", "strReplace", "insert"], "default": "create"},
                    "content": {"type": "string", "description": "Content to write (for create/insert)"},
                    "old_str": {"type": "string", "description": "String to find (for strReplace)"},
                    "new_str": {"type": "string", "description": "Replacement string (for strReplace)"},
                    "insert_line": {"type": "integer", "description": "Line number to insert at, 0-indexed (for insert)"}
                },
                "required": ["path"]
            }),
            requires_approval: true,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Filesystem]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'path'".to_string())?;
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("create");

        // EP-0019-03: write tools use the operator-configured SandboxConfig
        // to decide whether the target path is acceptable.
        let resolved = resolve_under_workspace(
            &self.workspace_root,
            path,
            &self.sandbox.write().await.writable_paths_resolved(&self.workspace_root),
            true,
        )
        .map_err(|e| format!("path: {e}"))?;

        match command {
            "create" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'content' for create".to_string())?;
                if let Some(parent) = resolved.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("mkdir: {e}"))?;
                }
                tokio::fs::write(&resolved, content)
                    .await
                    .map_err(|e| format!("write failed: {e}"))?;
                Ok(format!("wrote {} bytes to {}", content.len(), path))
            }
            "strReplace" => {
                let old_str = args
                    .get("old_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'old_str' for strReplace".to_string())?;
                let new_str = args
                    .get("new_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'new_str' for strReplace".to_string())?;

                let content = tokio::fs::read_to_string(&resolved)
                    .await
                    .map_err(|e| format!("read failed: {e}"))?;

                if !content.contains(old_str) {
                    return Err(format!("old_str not found in file '{}'", path));
                }

                let new_content = content.replacen(old_str, new_str, 1);
                tokio::fs::write(&resolved, &new_content)
                    .await
                    .map_err(|e| format!("write failed: {e}"))?;
                Ok(format!("replaced in {}", path))
            }
            "insert" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'content' for insert".to_string())?;
                let insert_line = args
                    .get("insert_line")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                let existing = tokio::fs::read_to_string(&resolved)
                    .await
                    .unwrap_or_default();
                let mut lines: Vec<&str> = existing.lines().collect();

                match insert_line {
                    Some(line) => {
                        let idx = line.min(lines.len());
                        // Insert content lines at position
                        let insert_lines: Vec<&str> = content.lines().collect();
                        for (i, l) in insert_lines.iter().enumerate() {
                            lines.insert(idx + i, l);
                        }
                    }
                    None => {
                        // Append at end
                        for l in content.lines() {
                            lines.push(l);
                        }
                    }
                }

                let new_content = lines.join("\n") + "\n";
                tokio::fs::write(&resolved, &new_content)
                    .await
                    .map_err(|e| format!("write failed: {e}"))?;
                Ok(format!("inserted into {} at line {:?}", path, insert_line))
            }
            other => Err(format!("unknown command '{}'", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sandbox(writable: Vec<String>) -> Arc<RwLock<Box<dyn SandboxConfig>>> {
        Arc::new(RwLock::new(Box::new(TestSandbox {
            enabled: true,
            writable,
        })))
    }

    struct TestSandbox {
        enabled: bool,
        writable: Vec<String>,
    }
    impl SandboxConfig for TestSandbox {
        fn enabled(&self) -> bool { self.enabled }
        fn is_writable(&self, path: &std::path::Path) -> bool {
            self.writable.iter().any(|w| path.starts_with(std::path::PathBuf::from(w)))
        }
        fn is_readable(&self, path: &std::path::Path) -> bool { self.is_writable(path) }
    }

    #[tokio::test]
    async fn write_file_create_writes_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = WriteFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({
                    "path": path.to_string_lossy().to_string(),
                    "command": "create",
                    "content": "hello world",
                }),
            )
            .await
            .expect("execute");
        assert!(result.contains("wrote"));
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written, "hello world");
    }

    #[tokio::test]
    async fn write_file_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = WriteFileTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({
                    "path": "/etc/test",
                    "command": "create",
                    "content": "x",
                }),
            )
            .await;
        assert!(result.is_err(), "expected error for path outside sandbox");
    }
}
