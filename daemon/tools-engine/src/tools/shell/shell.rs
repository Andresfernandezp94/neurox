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

        // EP-0011 shell sandbox enforcement: extract absolute paths from the
        // command and verify each is allowed under the configured paths.
        // READS (cat, ls, grep, head) require the path to be in
        // readable_paths; WRITES (>, >>, tee, rm, sed -i, cp dest, mv dest)
        // additionally require the path to be in writable_paths. This
        // stops the obvious `touch /etc/test` and `rm -rf /` cases
        // while still letting the LLM read /etc/passwd and similar.
        //
        // Best-effort: a clever command can still bypass via shell expansion
        // (e.g. `cat foo > bar` where bar is a glob). The parser only
        // checks the literal tokens.
        let sandbox = self.sandbox.read().await;
        let writable = sandbox.writable_paths_resolved(&self.workspace_root);
        let readable = sandbox.readable_paths_resolved(&self.workspace_root);
        let write_tokens = write_token_positions(command);
        let path_strs = extract_absolute_paths(command);
        // Match path strings back to token positions. We re-tokenize the
        // path to find it in the original command's token stream.
        let tokens = tokenize_command(command);
        for path_str in path_strs.iter() {
            // Find the token index whose value matches this path string.
            // If multiple matches, pick the first (the LLM passed the same
            // path twice; both should be treated the same way).
            let idx = tokens.iter().position(|t| t == path_str);
            let Some(idx) = idx else {
                // Path came from inside quotes that we stripped; treat
                // it as a write target (the safe default — better to ask
                // for confirmation than to allow a write).
                let p = std::path::Path::new(path_str);
                let resolved = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    self.workspace_root.join(p)
                };
                let normalized = normalize_path(&resolved);
                if !writable.iter().any(|root| normalized.starts_with(root)) {
                    return Err(format!(
                        "shell: path '{}' (quoted) is outside sandbox writable_paths",
                        normalized.display()
                    ));
                }
                continue;
            };
            let p = std::path::Path::new(path_str);
            let resolved = if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.workspace_root.join(p)
            };
            let normalized = normalize_path(&resolved);
            let is_write_target = write_tokens.contains(&idx);
            // EP-2026-09-01 safety net: skip empty path roots which would
            // match every input (the daemon should already filter them
            // in resolve_paths, but defense-in-depth in case a future
            // refactor breaks that).
            let allowed = if is_write_target {
                writable.iter().any(|root| !root.as_os_str().is_empty() && normalized.starts_with(root))
            } else {
                readable.iter().any(|root| !root.as_os_str().is_empty() && normalized.starts_with(root))
            };
            if !allowed {
                let kind = if is_write_target { "writable_paths" } else { "readable_paths" };
                return Err(format!(
                    "shell: path '{}' is outside sandbox {}",
                    normalized.display(),
                    kind
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

/// Tokenize a command respecting single/double quotes (same algorithm as
/// the sidebar's ShellSandbox.js so both implementations agree on what
/// counts as a "token").
fn tokenize_command(command: &str) -> Vec<String> {
    let mut buf = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut out: Vec<String> = Vec::new();
    for c in command.chars() {
        if c == '\'' && !in_double { in_single = !in_single; continue; }
        if c == '"' && !in_single { in_double = !in_double; continue; }
        if (c == ' ' || c == '\t' || c == '\n') && !in_single && !in_double {
            if !buf.is_empty() { out.push(std::mem::take(&mut buf)); }
            continue;
        }
        buf.push(c);
    }
    if !buf.is_empty() { out.push(buf); }
    out
}

/// Return the positions (token indices) of paths that are WRITE
/// targets of the command, based on simple shell-grammar heuristics.
/// Catches:
///   - Output redirection: `> FILE`, `>> FILE`, `[fd]>> FILE`
///   - `tee FILE`  → first non-flag arg is the file
///   - `rm FILE…` → every non-flag arg
///   - `mv SRC DST` / `cp SRC DST` → LAST non-flag arg (the destination)
///   - `sed -i FILE` → arg immediately after `-i` (or `-i.suffix`)
/// Best-effort: a clever command can still bypass via shell expansion.
/// Returns a HashSet of token indices for fast lookup.
fn write_token_positions(command: &str) -> std::collections::HashSet<usize> {
    use std::collections::HashSet;
    let tokens = tokenize_command(command);
    let mut out = HashSet::new();

    // First pass: `>` and `>>` redirections. The following token is
    // the destination. `>>` may have a leading fd number (e.g. `2>>`).
    for (i, t) in tokens.iter().enumerate() {
        let stripped = t.trim_start_matches(|c: char| c.is_ascii_digit());
        if stripped == ">" || stripped == ">>" {
            if let Some(next_idx) = i.checked_add(1) {
                if next_idx < tokens.len() {
                    out.insert(next_idx);
                }
            }
        }
    }

    // Second pass: write commands that take path args.
    // We treat any token that starts with '-' as a flag (including
    // compound forms like '-rf', '--recursive') and skip it. The
    // exception is bare '-' (stdin marker) which is rare enough to
    // ignore in our path-extraction approximation.
    for (i, t) in tokens.iter().enumerate() {
        match t.as_str() {
            // Simple "create or touch a file" commands
            "touch" | "mkdir" | "mkdir -p" => {
                for j in (i+1)..tokens.len() {
                    let tok = tokens[j].as_str();
                    if !tok.starts_with('-') {
                        out.insert(j);
                    }
                }
            }
            // Destructive: remove, unlink
            "rm" | "unlink" | "rmdir" | "rmdir -p" => {
                for j in (i+1)..tokens.len() {
                    let tok = tokens[j].as_str();
                    // Treat single '-' (stdin) and any --flag as flags.
                    if !tok.starts_with('-') || tok == "-" {
                        out.insert(j);
                    }
                }
            }
            // tee writes to its file arg(s)
            "tee" => {
                for j in (i+1)..tokens.len() {
                    if !tokens[j].starts_with('-') { out.insert(j); }
                }
            }
            // mv/cp: LAST non-flag arg is the destination (write target).
            "mv" | "cp" => {
                let mut last: Option<usize> = None;
                for j in (i+1)..tokens.len() {
                    if !tokens[j].starts_with('-') { last = Some(j); }
                }
                if let Some(j) = last { out.insert(j); }
            }
            // chmod/chown change perms/owner on a file
            "chmod" | "chown" => {
                // Last non-flag arg is the target
                let mut last: Option<usize> = None;
                for j in (i+1)..tokens.len() {
                    if !tokens[j].starts_with('-') { last = Some(j); }
                }
                if let Some(j) = last { out.insert(j); }
            }
            // sed -i FILE: arg after -i (or -i.suffix FILE) is mutated
            "sed" => {
                for j in (i+1)..tokens.len().min(i+6) {
                    if tokens[j].starts_with("-i") {
                        if let Some(ni) = j.checked_add(1) {
                            if ni < tokens.len() && !tokens[ni].starts_with('-') {
                                out.insert(ni);
                            }
                        }
                        break;
                    }
                }
            }
            // dd of=FILE writes to FILE. if=... of=... means output to of.
            "dd" => {
                for j in (i+1)..tokens.len() {
                    let tok = tokens[j].as_str();
                    if let Some(val) = tok.strip_prefix("of=") {
                        // Treat of=... as the destination — synthesize a
                        // path token at the same index so the existing
                        // path check fires on it.
                        if !val.is_empty() && val.starts_with('/') || val.starts_with('~') {
                            // We can't mutate `tokens` from here; instead
                            // we add the path to write_tokens by re-running
                            // the extract pass. Easier: just call back into
                            // extract_absolute_paths on this single value.
                            out.insert(j);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}
