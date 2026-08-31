use std::sync::Arc;
use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio::process::Command;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// GrepTool extracted from core/src/tools/mod.rs (tools/read/grep.rs)
pub struct GrepTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
    pub max_depth: usize,
}

#[async_trait]
impl Tool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".to_string(),
            description: "Search for a regex pattern in files. Returns matching lines with file:line:content format. Uses ripgrep if available.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex pattern (Rust regex syntax)"},
                    "path": {"type": "string", "default": "."},
                    "max_matches": {"type": "integer", "default": 100}
                },
                "required": ["pattern"]
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
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'pattern'".to_string())?;
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max = args
            .get("max_matches")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(100) as usize;

        // Try rg first
        if let Ok(output) = Command::new("rg")
            .arg("--no-heading")
            .arg("--line-number")
            .arg("--max-count")
            .arg(max.to_string())
            .arg(pattern)
            .arg(path)
            .current_dir(&self.workspace_root)
            .output()
            .await
        {
            if output.status.success() || output.status.code() == Some(1) {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if stdout.is_empty() {
                    return Ok("no matches".to_string());
                }
                return Ok(truncate_lines(&stdout, max));
            }
        }

        // Fallback: pure Rust regex + recursive walk
        let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
        let search_root = self.workspace_root.join(path);
        let mut results = Vec::new();
        grep_walk_dir(&search_root, &re, max, &mut results)?;
        if results.is_empty() {
            Ok("no matches".to_string())
        } else {
            Ok(truncate_lines(&results.join("\n"), max))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sandbox(readable: Vec<String>) -> Arc<tokio::sync::RwLock<Box<dyn SandboxConfig>>> {
        Arc::new(tokio::sync::RwLock::new(Box::new(TestSandbox {
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
    async fn grep_finds_matching_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\nworld\nhello again\n").unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = GrepTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
            max_depth: 8,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"pattern": "hello", "path": dir.path().to_string_lossy().to_string()}),
            )
            .await
            .expect("execute");
        assert!(result.contains("hello"), "should match: {result}");
        assert!(result.contains("hello again"));
        assert!(!result.contains("world"), "should NOT match world");
    }

    #[tokio::test]
    async fn grep_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = GrepTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
            max_depth: 8,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"pattern": "x", "path": "/etc"}),
            )
            .await;
        assert!(result.is_err());
    }
}
