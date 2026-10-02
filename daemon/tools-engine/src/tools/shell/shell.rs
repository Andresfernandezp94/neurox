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
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
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
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"command": "touch /etc/neurox-test-should-fail"}),
            )
            .await;
        assert!(result.is_err(), "shell outside sandbox should fail");
        let _ = std::fs::remove_file("/etc/neurox-test-should-fail");
    }

    // EP-2026-09-01 fixes S2 (bash both-redirects) + S3 (absolute-pathed binaries).
    // These four commands would have bypassed the sandbox before the fix and
    // written to /etc; they must now error out without touching the filesystem.
    async fn assert_write_blocked(tool: &ShellTool, cmd: &str) {
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"command": cmd}),
            )
            .await;
        let msg = result.as_ref().err().cloned().unwrap_or_default();
        assert!(result.is_err(), "expected Err for `{cmd}`, got: {msg}");
        assert!(
            msg.contains("writable_paths"),
            "expected sandbox error for `{cmd}`, got: {msg}"
        );
    }

    #[tokio::test]
    async fn shell_blocks_bash_both_redirect() {
        // S2 fix: `&>` is bash shorthand for `> FILE 2>&1`. Without the fix
        // the destination token is not flagged as a write target so it slips
        // through the readable_paths check.
        let sandbox = make_sandbox(vec![]);
        let tool = ShellTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
            timeout_secs: 5,
        };
        assert_write_blocked(&tool, "echo hi &> /etc/neurox-bypass-amp").await;
        assert_write_blocked(&tool, "echo hi &>> /etc/neurox-bypass-amp2").await;
        assert_write_blocked(&tool, "echo hi <> /etc/neurox-bypass-rw").await;
        let _ = std::fs::remove_file("/etc/neurox-bypass-amp");
        let _ = std::fs::remove_file("/etc/neurox-bypass-amp2");
        let _ = std::fs::remove_file("/etc/neurox-bypass-rw");
    }

    #[tokio::test]
    async fn shell_blocks_absolute_pathed_binaries() {
        // S3 fix: the write-command heuristic matches literal names like
        // `touch`. `/usr/bin/touch` was treated as a read path under /usr
        // and slipped past the writable check.
        //
        // The test sandbox mirrors the daemon's defaults: `/usr` is
        // readable (so the absolute path to the binary passes the read
        // check) but `/etc` is not writable (so the destination must be
        // blocked by the writable check).
        struct SandboxWithRead {
            writable: Vec<String>,
            readable: Vec<String>,
        }
        impl SandboxConfig for SandboxWithRead {
            fn enabled(&self) -> bool { true }
            fn is_writable(&self, path: &std::path::Path) -> bool {
                self.writable.iter().any(|w| path.starts_with(std::path::PathBuf::from(w)))
            }
            fn is_readable(&self, path: &std::path::Path) -> bool {
                self.readable.iter().any(|w| path.starts_with(std::path::PathBuf::from(w)))
                    || self.is_writable(path)
            }
            // ShellTool consults *paths_resolved* (not is_readable/is_writable
            // directly), so we must override these too. The default trait impl
            // returns empty Vec → everything gets blocked.
            fn writable_paths_resolved(&self, _ws: &std::path::Path) -> Vec<std::path::PathBuf> {
                self.writable.iter().map(std::path::PathBuf::from).collect()
            }
            fn readable_paths_resolved(&self, _ws: &std::path::Path) -> Vec<std::path::PathBuf> {
                self.readable.iter().map(std::path::PathBuf::from).collect()
            }
        }
        let sandbox = Arc::new(tokio::sync::RwLock::new(Box::new(SandboxWithRead {
            writable: vec!["/tmp".into()],
            readable: vec!["/usr".into(), "/etc".into(), "/tmp".into()],
        }) as Box<dyn SandboxConfig>));
        let tool = ShellTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
            timeout_secs: 5,
        };
        // `/usr/bin/touch /etc/neurox-bypass-abs`:
        //   - `/usr/bin/touch` is a read under /usr → OK
        //   - `/etc/neurox-bypass-abs` is a write target (basename=touch)
        //     and /etc is NOT in writable_paths → blocked.
        assert_write_blocked(&tool, "/usr/bin/touch /etc/neurox-bypass-abs").await;
        let _ = std::fs::remove_file("/etc/neurox-bypass-abs");
    }

    // S6 fix: empty / whitespace-only commands must error, not silently
    // succeed with an empty body. Without this the LLM can't tell whether
    // a no-op ran or whether the call was malformed.
    #[tokio::test]
    async fn shell_rejects_empty_command() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = ShellTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
            timeout_secs: 5,
        };
        for empty in ["", " ", "\t", "  \n  "] {
            let r = tool
                .execute(
                    &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                    serde_json::json!({"command": empty}),
                )
                .await;
            let msg = r.as_ref().err().cloned().unwrap_or_default();
            assert!(r.is_err(), "expected Err for empty input {empty:?}, got: {msg}");
            assert!(msg.contains("empty"), "expected 'empty' in error, got: {msg}");
        }
    }

    // S5 fix: tilde expansion. `~/projects/foo` must resolve to
    // $HOME/projects/foo before the sandbox check fires, instead of
    // being joined with workspace_root as a literal `~/...` path.
    #[tokio::test]
    async fn shell_expands_tilde_in_paths() {
        // Sandbox allows $HOME as both readable and writable, mimicking
        // the daemon's `readable_paths: [$HOME]` default.
        let home = std::env::var("HOME").expect("HOME unset");
        struct HomeSandbox { writable: Vec<String>, readable: Vec<String> }
        impl SandboxConfig for HomeSandbox {
            fn enabled(&self) -> bool { true }
            fn is_writable(&self, path: &std::path::Path) -> bool {
                self.writable.iter().any(|w| path.starts_with(std::path::PathBuf::from(w)))
            }
            fn is_readable(&self, path: &std::path::Path) -> bool {
                self.readable.iter().any(|w| path.starts_with(std::path::PathBuf::from(w)))
                    || self.is_writable(path)
            }
            fn writable_paths_resolved(&self, _ws: &std::path::Path) -> Vec<std::path::PathBuf> {
                self.writable.iter().map(std::path::PathBuf::from).collect()
            }
            fn readable_paths_resolved(&self, _ws: &std::path::Path) -> Vec<std::path::PathBuf> {
                self.readable.iter().map(std::path::PathBuf::from).collect()
            }
        }
        let sandbox = Arc::new(tokio::sync::RwLock::new(Box::new(HomeSandbox {
            writable: vec![home.clone()],
            readable: vec![home.clone()],
        }) as Box<dyn SandboxConfig>));
        let tool = ShellTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
            timeout_secs: 5,
        };
        // Write to ~/neurox-tilde-test.txt — must NOT be joined with
        // workspace_root as a literal "~/..." path.
        let target = format!("{home}/neurox-tilde-test.txt");
        let cmd = format!("echo hi > {target}");
        let r = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), workspace: None,
            cancel: None, http_client: None },
                serde_json::json!({"command": cmd}),
            )
            .await;
        assert!(r.is_ok(), "tilde write should succeed: {:?}", r);
        let body = std::fs::read_to_string(&target).unwrap_or_default();
        assert_eq!(body.trim(), "hi", "file contents wrong: {body:?}");
        let _ = std::fs::remove_file(&target);
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
                    "timeout_secs": {"type": "integer", "default": 30, "minimum": 1, "maximum": 30, "description": "Hard-capped to 30s (the daemon's shell tool timeout)."}
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
        // S6 fix: refuse empty / whitespace-only commands explicitly.
        // Without this they reach /bin/sh -c "" which silently succeeds
        // and returns ok:true with an empty body — ambiguous for the LLM
        // (did the command produce no output or was it not executed?).
        if command.trim().is_empty() {
            return Err("shell: empty command".into());
        }
        let timeout = args
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(self.timeout_secs)
            .clamp(1, self.timeout_secs);

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
        let scope = ctx.scope(&self.workspace_root, &self.sandbox);
        let writable = scope.writable().await;
        let readable = scope.readable().await;
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
            // S5 fix: expand `~` BEFORE the workspace_root join so that
            // `~/foo` resolves to `$HOME/foo` and not to a literal
            // `/workspace_root/~/foo` path. See the equivalent block above.
            let p = if let Some(rest) = p.to_str() {
                if let Some(stripped) = rest.strip_prefix("~/") {
                    if let Some(home) = std::env::var_os("HOME") {
                        std::path::PathBuf::from(home).join(stripped)
                    } else {
                        p.to_path_buf()
                    }
                } else if rest == "~" {
                    if let Some(home) = std::env::var_os("HOME") {
                        std::path::PathBuf::from(home)
                    } else {
                        p.to_path_buf()
                    }
                } else {
                    p.to_path_buf()
                }
            } else {
                p.to_path_buf()
            };
            let resolved = if p.is_absolute() {
                p.clone()
            } else {
                scope.root.join(&p)
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
            // S5 fix: expand a leading `~` BEFORE the absolute-or-relative
            // join, otherwise `~/projects/foo` gets joined with
            // workspace_root as `/home/<user>/~/projects/foo` which is a
            // literal non-existent path. We rely on normalize_path to also
            // handle the case where the path is already expanded (it's a
            // no-op then).
            let p = if let Some(rest) = p.to_str() {
                if let Some(stripped) = rest.strip_prefix("~/") {
                    if let Some(home) = std::env::var_os("HOME") {
                        std::path::PathBuf::from(home).join(stripped)
                    } else {
                        p.to_path_buf()
                    }
                } else if rest == "~" {
                    if let Some(home) = std::env::var_os("HOME") {
                        std::path::PathBuf::from(home)
                    } else {
                        p.to_path_buf()
                    }
                } else {
                    p.to_path_buf()
                }
            } else {
                p.to_path_buf()
            };
            let resolved = if p.is_absolute() {
                p.clone()
            } else {
                self.workspace_root.join(&p)
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
            .current_dir(scope.root)
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

/// EP-0011 shell sandbox helper: normalize a path by expanding a
/// leading `~` (using $HOME) and collapsing `.` / `..` segments.
/// Doesn't touch the filesystem (so symlinks aren't resolved — that's
/// the caller's job if needed).
fn normalize_path(p: &std::path::Path) -> std::path::PathBuf {
    // S5 fix: tilde expansion. Without this, `~/projects/foo` is
    // joined with workspace_root as `/home/<user>/~/projects/foo`
    // and rejected with a confusing "outside sandbox" error.
    let expanded = if let Some(rest) = p.to_str() {
        if let Some(stripped) = rest.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                std::path::PathBuf::from(home).join(stripped)
            } else {
                p.to_path_buf()
            }
        } else if rest == "~" {
            if let Some(home) = std::env::var_os("HOME") {
                std::path::PathBuf::from(home)
            } else {
                p.to_path_buf()
            }
        } else {
            p.to_path_buf()
        }
    } else {
        p.to_path_buf()
    };

    let mut out = std::path::PathBuf::new();
    for comp in expanded.components() {
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
    // `&>`, `&>>` (bash both-streams redirect) and `<>` (read-write
    // redirect) also write to the following token.
    for (i, t) in tokens.iter().enumerate() {
        let stripped = t.trim_start_matches(|c: char| c.is_ascii_digit());
        if stripped == ">" || stripped == ">>" {
            if let Some(next_idx) = i.checked_add(1) {
                if next_idx < tokens.len() {
                    out.insert(next_idx);
                }
            }
        } else if t == "&>" || t == "&>>" || stripped == "<>" {
            // Bash `&>` / `&>>` redirect both stdout and stderr; the
            // following token is the destination. `<>` opens the file
            // for read-write (truncating), so the following token is
            // also a write target.
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
    //
    // We resolve each token to its `basename` before matching, so that
    // absolute-pathed binaries (`/usr/bin/touch`, `./touch`) are
    // recognized the same as the bare command name.
    for (i, t) in tokens.iter().enumerate() {
        let cmd_name = std::path::Path::new(t)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(t);
        match cmd_name {
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
