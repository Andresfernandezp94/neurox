use std::sync::Arc;
use std::path::PathBuf;
use serde_json::Value;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio::process::Command;
use std::time::Duration;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// ShellTool extracted from core/src/tools/mod.rs (tools/shell/shell.rs)
pub struct ShellTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
    pub timeout_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sandbox(writable: Vec<String>) -> Arc<tokio::sync::RwLock<Box<dyn SandboxConfig>>> {
        Arc::new(tokio::sync::RwLock::new(Box::new(TestSandbox {
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
    async fn shell_executes_basic_command() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ShellTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
            timeout_secs: 5,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"command": "echo hello"}),
            )
            .await
            .expect("execute");
        assert!(result.contains("hello"), "got: {result}");
    }

    #[tokio::test]
    async fn shell_outside_sandbox_errors() {
        // EP-0011: shell.rs now extracts absolute paths from the command
        // and verifies each is under writable_paths before executing.
        let sandbox = make_sandbox(vec![]);
        let tool = ShellTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
            timeout_secs: 5,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({"command": "touch /etc/neurox-test-should-fail"}),
            )
            .await;
        assert!(result.is_err(), "shell outside sandbox should fail");
        let _ = std::fs::remove_file("/etc/neurox-test-should-fail");
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".to_string(),
            description:
                "Execute a shell command via /bin/sh -c, with a timeout. Returns stdout+stderr."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Shell command to execute"},
                    "timeout_secs": {"type": "integer", "default": 30, "maximum": 600}
                },
                "required": ["command"]
            }),
            requires_approval: true,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Shell]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build]
    }


    async fn execute(&self, ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'command'".to_string())?;
        let timeout = args
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(self.timeout_secs)
            .min(self.timeout_secs);

        // EP-0011 shell sandbox enforcement: extract absolute paths from
        // the command and verify each is under the configured
        // writable_paths. This is a best-effort check (a clever command
        // can still bypass it) but it stops the obvious cases like
        // `touch /etc/test` or `rm -rf /`.
        let writable = self
            .sandbox
            .read()
            .await
            .writable_paths_resolved(&self.workspace_root);
        for path_str in extract_absolute_paths(command) {
            let p = std::path::Path::new(&path_str);
            let resolved = if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.workspace_root.join(p)
            };
            let normalized = normalize_path(&resolved);
            let allowed = writable.iter().any(|root| normalized.starts_with(root));
            if !allowed {
                return Err(format!(
                    "shell: path '{}' is outside sandbox writable_paths",
                    normalized.display()
                ));
            }
        }

        // EP-0013 T-003: respect cancellation token if provided.
        let cancel = ctx.cancel.clone();
        let cmd_future = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.workspace_root)
            .output();
        let result = if let Some(token) = cancel {
            tokio::select! {
                res = tokio::time::timeout(Duration::from_secs(timeout), cmd_future) => {
                    res.map_err(|_| format!("timeout after {timeout}s"))?
                       .map_err(|e| format!("exec failed: {e}"))?
                }
                _ = token.cancelled() => {
                    return Err("shell command cancelled".into());
                }
            }
        } else {
            tokio::time::timeout(Duration::from_secs(timeout), cmd_future)
                .await
                .map_err(|_| format!("timeout after {timeout}s"))?
                .map_err(|e| format!("exec failed: {e}"))?
        };

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        let mut out = String::new();
        if !stdout.is_empty() {
            out.push_str(&stdout);
        }
        if !stderr.is_empty() {
            out.push_str("\n[stderr]\n");
            out.push_str(&stderr);
        }
        if !result.status.success() {
            out.push_str(&format!("\n[exit code: {:?}]", result.status.code()));
        }
        Ok(truncate_lines(&out, 200))
    }
}

/// EP-0011 shell sandbox helper: extract all path-like tokens from a
/// shell command. Best-effort: catches absolute paths and quoted
/// relative paths. Doesn't try to be a full shell parser — that
/// would require integrating `shell-words` or similar. The point is
/// to stop the obvious cases (`touch /etc/test`, `rm -rf /`).
fn extract_absolute_paths(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    for c in command.chars() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ' ' | '\t' | '\n' if !in_single && !in_double => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.into_iter()
        .filter(|t| t.starts_with('/') || t.starts_with('~'))
        .filter(|t| !t.contains("://")) // skip URLs
        .filter(|t| !t.contains('*') && !t.contains('?')) // skip globs
        .collect()
}

/// EP-0011 shell sandbox helper: normalize a path by collapsing `.`
/// and `..` segments. Doesn't touch the filesystem (so symlinks aren't
/// resolved — that's the caller's job if needed).
fn normalize_path(p: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
