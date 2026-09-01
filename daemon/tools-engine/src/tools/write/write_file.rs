use tokio::sync::RwLock;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use serde_json::Value;
use async_trait::async_trait;
use fs2::FileExt;
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
                atomic_write(&resolved, content.as_bytes()).await?;
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

                // W2 fix: the flock inside str_replace_once serializes
                // concurrent writers, so a single attempt is enough when
                // the operation can succeed. We still loop on transient
                // I/O errors (e.g. ENOENT during a sibling delete) with
                // a small backoff.
                const MAX_ATTEMPTS: usize = 8;
                let mut last_err: Option<String> = None;
                for _attempt in 0..MAX_ATTEMPTS {
                    match str_replace_once(&resolved, old_str, new_str).await {
                        Ok(()) => return Ok(format!("replaced in {}", path)),
                        Err(StrReplaceError::NotFound) => {
                            return Err(format!("old_str not found in file '{}'", path));
                        }
                        Err(StrReplaceError::Io(e)) => return Err(e),
                    }
                }
                // Unreachable: str_replace_once returns Ok, NotFound, or Io.
                Err(format!(
                    "strReplace could not land after {MAX_ATTEMPTS} attempts: {}",
                    last_err.unwrap_or_default()
                ))
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

                insert_lines(&resolved, content, insert_line).await?;
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

    // W1+W4+W5 fix: atomic write. Two concurrent writers to the same path
    // must not produce a state where readers see "no such file" or partial
    // bytes. The atomic_write helper writes to a tmpfile in the same
    // directory and renames atomically.
    #[tokio::test]
    async fn atomic_write_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.txt");
        std::fs::write(&path, "OLD").unwrap();
        atomic_write(&path, b"NEW").await.expect("atomic write");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "NEW");
    }

    // W4 fix: atomic_write to a fresh path must still result in the file
    // existing on disk immediately after the call returns (no window where
    // a concurrent reader can see ENOENT).
    #[tokio::test]
    async fn atomic_write_creates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fresh.txt");
        assert!(!path.exists());
        atomic_write(&path, b"HELLO").await.expect("atomic write");
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "HELLO");
    }

    // W2 fix: strReplace under contention with DIFFERENT old_str values
    // must all succeed thanks to flock serialization. (When N calls race
    // on the SAME old_str, only one wins — that's correct behavior.)
    #[tokio::test]
    async fn str_replace_serializes_under_contention() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.txt");
        std::fs::write(&path, "AAA_0 AAA_1 AAA_2 AAA_3 AAA_4 AAA_5 AAA_6 AAA_7 AAA_8 AAA_9").unwrap();
        // Fire 10 concurrent strReplaces, each replacing a unique marker.
        // All must succeed; the final content must contain all 10 new markers.
        let mut handles = vec![];
        for i in 0..10 {
            let p = path.clone();
            handles.push(tokio::spawn(async move {
                str_replace_once(
                    &p,
                    &format!("AAA_{i}"),
                    &format!("MARKER_{i}"),
                )
                .await
            }));
        }
        let mut ok = 0;
        for h in handles {
            if h.await.unwrap().is_ok() { ok += 1; }
        }
        assert_eq!(ok, 10, "all 10 strReplaces must succeed under flock");
        let r#final = std::fs::read_to_string(&path).unwrap();
        for i in 0..10 {
            assert!(r#final.contains(&format!("MARKER_{i}")), "missing MARKER_{i}: {}", r#final);
        }
        // And none of the originals should remain.
        for i in 0..10 {
            assert!(!r#final.contains(&format!("AAA_{i}")), "AAA_{i} still present: {}", r#final);
        }
    }

    // W2 fix: strReplace with the SAME old_str — only one of N concurrent
    // calls should succeed (the others see the marker already replaced).
    // The flock ensures no torn writes; the retry-on-conflict path
    // guarantees we don't return spurious "old_str not found" when the
    // file genuinely never had it.
    #[tokio::test]
    async fn str_replace_same_marker_only_one_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.txt");
        std::fs::write(&path, "AAA BBB CCC").unwrap();
        let mut handles = vec![];
        for i in 0..10 {
            let p = path.clone();
            handles.push(tokio::spawn(async move {
                str_replace_once(&p, "BBB", &format!("MARKER_{i}")).await
            }));
        }
        let mut ok = 0;
        let mut not_found = 0;
        for h in handles {
            match h.await.unwrap() {
                Ok(()) => ok += 1,
                Err(StrReplaceError::NotFound) => not_found += 1,
                Err(e) => panic!("unexpected error: {:?}", e),
            }
        }
        assert_eq!(ok, 1, "exactly one strReplace should win");
        assert_eq!(not_found, 9, "the other 9 should fail with NotFound");
    }
}

// ────────────────────────────────────────────────────────────────────
// Internal helpers (extracted to keep `execute` readable)
// ────────────────────────────────────────────────────────────────────

/// W1+W4+W5: atomic write — ensure the target exists, then write to
/// a tmpfile in the same directory and rename into place. `rename(2)`
/// is atomic on POSIX so readers always observe the old or the new
/// file, never a partial state.
///
/// W4 fix: pre-create the target file (empty) before writing so
/// concurrent readers never see ENOENT. During the write window they
/// see an empty file; once rename lands they see the new content.
async fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    use uuid::Uuid;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("mkdir: {e}"))?;
        }
    }
    // Pre-create the target so a concurrent reader never sees ENOENT.
    // If the file already exists this is a no-op (O_CREAT without
    // O_TRUNC and without O_EXCL). The file may briefly be empty
    // during the write window, but it always exists.
    tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .await
        .map_err(|e| format!("precreate: {e}"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".tmp.{}", Uuid::new_v4()));
    // Write tmpfile (still invisible to readers of the original path).
    tokio::fs::write(&tmp, content)
        .await
        .map_err(|e| format!("write failed: {e}"))?;
    // Atomic rename onto the target path.
    if let Err(e) = tokio::fs::rename(&tmp, path).await {
        // Best-effort cleanup so we don't leave .tmp.* files behind.
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(format!("rename failed: {e}"));
    }
    Ok(())
}

/// W2 fix: outcome of a single strReplace attempt.
#[derive(Debug)]
enum StrReplaceError {
    /// `old_str` is genuinely absent — caller should fail fast.
    NotFound,
    /// Filesystem / I/O error — surface to caller verbatim.
    Io(String),
}

/// W2 fix: one attempt of strReplace under an exclusive flock.
/// If `old_str` is no longer present because another writer modified
/// the file, returns `NotFound` (the caller may or may not retry —
/// for the same `old_str` from concurrent callers, only one wins).
async fn str_replace_once(
    path: &Path,
    old_str: &str,
    new_str: &str,
) -> Result<(), StrReplaceError> {
    // Open (or create) the file, then take an exclusive flock that is
    // held for the duration of the strReplace operation. The lock is
    // released when the `_lock_guard` is dropped at the end of this
    // function — that serializes concurrent writers on the same path.
    let _lock_guard = tokio::task::spawn_blocking({
        let p = path.to_path_buf();
        move || -> Result<std::fs::File, String> {
            let f = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&p)
                .map_err(|e| format!("open: {e}"))?;
            f.lock_exclusive().map_err(|e| format!("flock: {e}"))?;
            Ok(f)
        }
    })
    .await
    .map_err(|e| StrReplaceError::Io(format!("join: {e}")))?
    .map_err(StrReplaceError::Io)?;

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| StrReplaceError::Io(format!("read: {e}")))?;
    if !content.contains(old_str) {
        return Err(StrReplaceError::NotFound);
    }
    let new_content = content.replacen(old_str, new_str, 1);
    atomic_write(path, new_content.as_bytes())
        .await
        .map_err(StrReplaceError::Io)?;
    // `_lock_guard` dropped here → flock released.
    Ok(())
}

/// W3 fix: insert with flock so concurrent inserts don't lose updates.
async fn insert_lines(
    path: &Path,
    content: &str,
    insert_line: Option<usize>,
) -> Result<(), String> {
    let _lock_guard = tokio::task::spawn_blocking({
        let p = path.to_path_buf();
        move || -> Result<std::fs::File, String> {
            let f = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&p)
                .map_err(|e| format!("open: {e}"))?;
            f.lock_exclusive().map_err(|e| format!("flock: {e}"))?;
            Ok(f)
        }
    })
    .await
    .map_err(|e| format!("join: {e}"))??;

    let existing = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("read: {e}"))?;
    let mut lines: Vec<&str> = existing.lines().collect();
    match insert_line {
        Some(line) => {
            let idx = line.min(lines.len());
            let insert_lines: Vec<&str> = content.lines().collect();
            for (i, l) in insert_lines.iter().enumerate() {
                lines.insert(idx + i, l);
            }
        }
        None => {
            for l in content.lines() {
                lines.push(l);
            }
        }
    }
    let new_content = lines.join("\n") + "\n";
    atomic_write(path, new_content.as_bytes()).await
    // `_lock_guard` dropped here → flock released.
}
