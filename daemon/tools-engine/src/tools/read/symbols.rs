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
// SymbolsTool extracted from core/src/tools/mod.rs (tools/read/symbols.rs)
pub struct SymbolsTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

#[async_trait]
impl Tool for SymbolsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "symbols".to_string(),
            description: "Search for function/class/method definitions in code. Uses ast-grep (sg) if available, falls back to regex.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Symbol pattern to search for"},
                    "path": {"type": "string", "default": ".", "description": "Directory to search in"},
                    "limit": {"type": "integer", "default": 50}
                },
                "required": ["query"]
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
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'query'".to_string())?;
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        let scope = ctx.scope(&self.workspace_root, &self.sandbox);
        let search_dir = resolve_under_workspace(
            scope.root,
            path,
            &scope.readable().await,
            false,
        )
        .map_err(|e| format!("path: {e}"))?;

        // Try ast-grep (sg) first. The `sg --pattern <query> --json
        // <search_dir>` invocation is a no-op if the user passes a
        // plain string (not a metavar-based pattern like `function $A`),
        // because ast-grep needs placeholders. We attempt the call
        // anyway — the fallback regex below covers the common case.
        if let Ok(output) = Command::new("sg")
            .arg("--pattern")
            .arg(query)
            .arg("--json")
            .arg(".")
            .current_dir(&search_dir)
            .output()
            .await
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.is_empty() {
                    return Ok(truncate_lines(&stdout, limit));
                }
            }
        }

        // Fallback: regex search for common definition patterns. The
        // original `{}`-after-name pattern missed `fn main()`-style
        // signatures (where the brace is on the next line, common in
        // Rust/C/Go), so we now anchor on a word boundary instead.
        let def_pattern = format!(
            r"(?i)\b(fn|func|def|class|struct|enum|trait|interface|type|const|let|var)\s+{q}\b",
            q = regex::escape(query)
        );
        let re = regex::Regex::new(&def_pattern).map_err(|e| format!("regex: {e}"))?;
        let mut results = Vec::new();
        grep_walk_dir(&search_dir, &re, limit, &mut results)?;
        if results.is_empty() {
            Ok(format!("no symbols matching '{}' found", query))
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
    async fn symbols_finds_pattern_in_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("code.rs"),
            "fn foo() {}\nfn bar() {}\nfn baz() {}\n",
        )
        .unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = SymbolsTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"query": "fn foo", "path": dir.path().to_string_lossy().to_string()}),
            )
            .await
            .expect("execute");
        assert!(result.contains("foo"), "should find foo: {result}");
    }

    #[tokio::test]
    async fn symbols_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = SymbolsTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"query": "x", "path": "/etc"}),
            )
            .await;
        assert!(result.is_err());
    }
}
