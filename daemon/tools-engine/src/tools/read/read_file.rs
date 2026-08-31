use tokio::sync::RwLock;
use std::sync::Arc;
use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// ReadFileTool extracted from core/src/tools/mod.rs (tools/read/read_file.rs)
pub struct ReadFileTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

#[async_trait]
impl Tool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".to_string(),
            description: "Read the contents of a UTF-8 text file. Shows line numbers. For large files (>500 lines), returns first 300 unless offset/limit specified.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path to the file"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Starting line number (0-indexed). Omit to start from beginning."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of lines to read. Omit to read all (subject to auto-truncation)."
                    }
                },
                "required": ["path"]
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
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'path'".to_string())?;
        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let resolved = resolve_under_workspace(
            &self.workspace_root,
            path,
            &self.sandbox.read().await.readable_paths_resolved(&self.workspace_root),
            false,
        )
        .map_err(|e| format!("path: {e}"))?;
        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| format!("read failed: {e}"))?;

        let all_lines: Vec<&str> = content.lines().collect();
        let total = all_lines.len();

        let (start, end) = match (offset, limit) {
            (Some(o), Some(l)) => (o, (o + l).min(total)),
            (Some(o), None) => (o, total),
            (None, Some(l)) => (0, l.min(total)),
            (None, None) => {
                if total > 500 {
                    // Auto-truncate: show first 300 + message
                    let mut out = String::with_capacity(300 * 80);
                    for (i, line) in all_lines.iter().take(300).enumerate() {
                        out.push_str(&format!("{}| {}\n", i + 1, line));
                    }
                    out.push_str(&format!(
                        "\n[File has {} lines. Use offset/limit to read ranges.]",
                        total
                    ));
                    return Ok(out);
                }
                (0, total)
            }
        };

        let mut out = String::with_capacity((end - start) * 80);
        for (i, line) in all_lines.iter().enumerate().take(end).skip(start) {
            if i < total {
                out.push_str(&format!("{}| {}\n", i + 1, line));
            }
        }
        if out.is_empty() {
            out = format!(
                "[No lines in range {}-{}. File has {} lines.]",
                start, end, total
            );
        }
        Ok(out)
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
        fn enabled(&self) -> bool {
            self.enabled
        }
        fn is_writable(&self, _: &std::path::Path) -> bool {
            false
        }
        fn is_readable(&self, path: &std::path::Path) -> bool {
            self.readable.iter().any(|r| {
                let p = std::path::PathBuf::from(r);
                path.starts_with(&p) || path == p
            })
        }
    }

    #[tokio::test]
    async fn read_file_returns_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ReadFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"path": path.to_string_lossy().to_string()}),
            )
            .await
            .expect("execute");
        assert!(result.contains("line1"));
        assert!(result.contains("line3"));
    }

    #[tokio::test]
    async fn read_file_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = ReadFileTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"path": "/etc/passwd"}),
            )
            .await;
        assert!(result.is_err(), "expected error for path outside sandbox");
    }
}
