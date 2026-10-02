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


    async fn execute(&self, ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'path'".to_string())?;
        if path.is_empty() {
            return Err("read_file: empty path".into());
        }
        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let scope = ctx.scope(&self.workspace_root, &self.sandbox);
        let resolved = resolve_under_workspace(
            scope.root,
            path,
            &scope.readable().await,
            false,
        )
        .map_err(|e| format!("path: {e}"))?;

        // R-W4 fix: a concurrent `write_file create` may briefly leave the
        // file missing or empty between the pre-create open and the
        // rename. Retry on ENOENT with a short backoff to bridge the
        // async race window. Other errors surface immediately.
        let mut last_err: Option<String> = None;
        let mut content: String = String::new();
        for attempt in 0..5u32 {
            match tokio::fs::read_to_string(&resolved).await {
                Ok(s) => { content = s; last_err = None; break; }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    last_err = Some(format!("read failed: {e}"));
                    tokio::time::sleep(std::time::Duration::from_millis(5 * (attempt as u64 + 1))).await;
                }
                Err(e) => return Err(format!("read failed: {e}")),
            }
        }
        if let Some(e) = last_err {
            return Err(e);
        }

        let all_lines: Vec<&str> = content.lines().collect();
        let total = all_lines.len();

        let (start, end) = match (offset, limit) {
            (Some(o), Some(l)) => (o.min(total), o.saturating_add(l).min(total)),
            (Some(o), None) => (o.min(total), total),
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

        let mut out = String::with_capacity(end.saturating_sub(start) * 80);
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
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
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
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"path": "/etc/passwd"}),
            )
            .await;
        assert!(result.is_err(), "expected error for path outside sandbox");
    }

    // R-W4 fix: capacity-overflow panic when offset > total and no limit
    // is given (start=offset, end=total, end-start underflowed usize).
    #[tokio::test]
    async fn read_file_offset_past_end_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.txt");
        std::fs::write(&path, "line1").unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ReadFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"path": path.to_string_lossy().to_string(), "offset": 100}),
            )
            .await;
        // Pre-fix: capacity-overflow panic crashed the daemon.
        // Post-fix: returns Ok with an empty-range message.
        assert!(result.is_ok(), "must not panic: {:?}", result);
    }

    // R-fix: empty path must return a clear error, not "Is a directory".
    #[tokio::test]
    async fn read_file_empty_path_errors() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ReadFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"path": ""}),
            )
            .await;
        let msg = result.as_ref().err().cloned().unwrap_or_default();
        assert!(result.is_err(), "expected Err for empty path");
        assert!(msg.contains("empty"), "got: {msg}");
    }

    // R-fix: limit=0 should not panic and should not silently return empty.
    #[tokio::test]
    async fn read_file_limit_zero_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.txt");
        std::fs::write(&path, "a\nb\nc\n").unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ReadFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"path": path.to_string_lossy().to_string(), "limit": 0}),
            )
            .await;
        assert!(result.is_ok(), "limit=0 must not panic: {:?}", result);
    }
}
