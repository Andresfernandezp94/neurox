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
// GlobTool extracted from core/src/tools/mod.rs (tools/read/glob.rs)
pub struct GlobTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
    pub max_depth: usize,
}

#[async_trait]
impl Tool for GlobTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".to_string(),
            description: "Search for files by name/glob pattern. Uses fd if available, falls back to recursive walk.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "File name pattern (glob syntax, e.g. '*.rs', '**/*.toml')"},
                    "path": {"type": "string", "default": ".", "description": "Directory to search in"},
                    "max_results": {"type": "integer", "default": 50}
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
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        let search_dir = resolve_under_workspace(
            &self.workspace_root,
            path,
            &self.sandbox.read().await.readable_paths_resolved(&self.workspace_root),
            false,
        )
        .map_err(|e| format!("path: {e}"))?;

        // Try fd first
        if let Ok(output) = Command::new("fd")
            .arg("--glob")
            .arg(pattern)
            .arg("--max-results")
            .arg(max_results.to_string())
            .arg(".")
            .current_dir(&search_dir)
            .output()
            .await
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if stdout.is_empty() {
                    return Ok("no matches".to_string());
                }
                return Ok(stdout);
            }
        }

        // Fallback: recursive walk with glob matching
        let glob_pattern =
            glob::Pattern::new(pattern).map_err(|e| format!("invalid glob pattern: {e}"))?;
        let mut results = Vec::new();
        glob_walk_dir(
            &search_dir,
            &glob_pattern,
            max_results,
            &search_dir,
            &mut results,
        );
        if results.is_empty() {
            Ok("no matches".to_string())
        } else {
            Ok(results.join("\n"))
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
    async fn glob_finds_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        std::fs::write(dir.path().join("b.md"), "x").unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = GlobTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
            max_depth: 8,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"pattern": "*.txt", "path": dir.path().to_string_lossy().to_string()}),
            )
            .await
            .expect("execute");
        assert!(result.contains("a.txt"), "missing a.txt: {result}");
        assert!(!result.contains("b.md"), "should NOT match b.md: {result}");
    }

    #[tokio::test]
    async fn glob_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = GlobTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
            max_depth: 8,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"pattern": "*", "path": "/etc"}),
            )
            .await;
        assert!(result.is_err());
    }
}
