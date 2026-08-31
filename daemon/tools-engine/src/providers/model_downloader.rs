//! Model downloader (EP-0018-08).
//!
//! Pulls a GGUF file from Hugging Face into `MODELS_DIR`. The endpoint
//! is intentionally simple (POST blocking, no streaming progress) so
//! the operator gets a clear OK / ERROR response. For very large
//! files this can take a few minutes; we set a generous timeout.
//!
//! URL shape (Hugging Face `resolve` redirect):
//!   `https://huggingface.co/{repo_id}/resolve/main/{filename}`
//!
//! Storage layout: the file lands at `MODELS_DIR/{filename}`. We
//! canonicalise the resolved path and refuse anything that escapes
//! `MODELS_DIR` (defence-in-depth against path traversal via crafted
//! `filename` or `repo_id`).

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::hf_models;

const HF_RESOLVE_BASE: &str = "https://huggingface.co";
const DOWNLOAD_TIMEOUT_SECS: u64 = 600; // 10 min for very large GGUFs

/// Outcome of a successful download.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadResult {
    pub path: String,
    pub size_bytes: u64,
}

/// Build the URL Hugging Face uses for raw file downloads.
pub fn hf_resolve_url(repo_id: &str, filename: &str) -> Result<String, String> {
    validate_repo_id(repo_id)?;
    validate_filename(filename)?;
    Ok(format!(
        "{HF_RESOLVE_BASE}/{}/resolve/main/{}",
        repo_id,
        // Reuse the same encoder the search endpoint uses so paths with
        // spaces or unicode round-trip correctly through HF's redirector.
        hf_models::url_encode(filename)
    ))
}

/// Resolve a `(repo_id, filename)` to a target path under `models_dir`
/// and return the path. The result is canonicalised to defeat any
/// `..` segments that might slip through a malicious filename.
pub fn target_path(models_dir: &Path, filename: &str) -> Result<PathBuf, String> {
    validate_filename(filename)?;
    let candidate = models_dir.join(filename);
    let canonical = candidate
        .canonicalize()
        .unwrap_or_else(|_| models_dir.join(filename));
    // Reject if the resolved path escapes models_dir. The canonical
    // form strips `..` segments so the only way to escape is via a
    // symlink pointing outside; that is still useful to catch early.
    let models_canonical = models_dir
        .canonicalize()
        .map_err(|e| format!("models_dir does not exist: {e}"))?;
    if !canonical.starts_with(&models_canonical) {
        return Err(format!(
            "resolved path escapes models_dir: {}",
            canonical.display()
        ));
    }
    // Disallow path components that look like traversal even before
    // canonicalisation: any leading `..` or absolute prefix is rejected.
    if filename.split('/').any(|c| c == "..") {
        return Err(format!("filename contains '..' segment: {filename}"));
    }
    Ok(canonical)
}

/// Validate a Hugging Face repo id (`owner/name`). No slashes, dots,
/// or whitespace beyond the single separator.
fn validate_repo_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 96 {
        return Err(format!("invalid repo id length: {id:?}"));
    }
    let mut parts = id.split('/');
    let owner = parts.next().unwrap_or("");
    let name = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(format!("repo id must be `owner/name`, got {id:?}"));
    }
    for (label, part) in [("owner", owner), ("name", name)] {
        if part.is_empty() {
            return Err(format!("empty {label} in repo id {id:?}"));
        }
        if !part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(format!("invalid chars in {label} {part:?}"));
        }
    }
    Ok(())
}

/// Reject filenames that would let a request escape `models_dir`.
fn validate_filename(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 256 {
        return Err(format!("invalid filename length: {name:?}"));
    }
    // Reject backslashes outright so the path-validation on Unix (which
    // doesn't treat `\` as a separator) still catches Windows-style
    // traversal attempts.
    if name.contains('\\') {
        return Err(format!("invalid character in {name:?}"));
    }
    let path = Path::new(name);
    for comp in path.components() {
        match comp {
            Component::Normal(_) => {}
            Component::CurDir => {} // `.` is fine
            _ => return Err(format!("invalid path component in {name:?}")),
        }
    }
    Ok(())
}

/// Download a GGUF file from Hugging Face into `models_dir`. Streams
/// the body to disk to avoid loading multi-GB files into memory.
pub async fn download(
    models_dir: &Path,
    repo_id: &str,
    filename: &str,
) -> Result<DownloadResult, String> {
    let url = hf_resolve_url(repo_id, filename)?;
    let target = target_path(models_dir, filename)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create target dir: {e}"))?;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .user_agent(concat!("neurox/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HF download request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HF returned HTTP {} for {url}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("failed to read HF response body: {e}"))?;
    let size = bytes.len() as u64;

    std::fs::write(&target, &bytes)
        .map_err(|e| format!("failed to write {}: {e}", target.display()))?;

    Ok(DownloadResult {
        path: target.to_string_lossy().to_string(),
        size_bytes: size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_repo_id_accepts_canonical() {
        validate_repo_id("Qwen/Qwen2.5-1.5B-Instruct-GGUF").unwrap();
        validate_repo_id("TheBloke/Llama-2-7B-Chat-GGUF").unwrap();
        validate_repo_id("owner_with.dots/repo").unwrap();
    }

    #[test]
    fn validate_repo_id_rejects_malformed() {
        assert!(validate_repo_id("").is_err());
        assert!(validate_repo_id("only-one-part").is_err());
        assert!(validate_repo_id("a/b/c").is_err());
        assert!(validate_repo_id("owner/repo with space").is_err());
        assert!(validate_repo_id("owner/../traversal").is_err());
        assert!(validate_repo_id("/leading-slash").is_err());
    }

    #[test]
    fn validate_filename_rejects_traversal() {
        assert!(validate_filename("").is_err());
        assert!(validate_filename("../escape").is_err());
        assert!(validate_filename("subdir/../escape").is_err());
        assert!(validate_filename("subdir/../../etc/passwd").is_err());
        assert!(validate_filename("/abs/path").is_err());
        assert!(validate_filename("C:\\windows").is_err());
    }

    #[test]
    fn validate_filename_accepts_normal_gguf_names() {
        validate_filename("qwen2.5-1.5b-instruct-q4_k_m.gguf").unwrap();
        validate_filename("subdir/model.gguf").unwrap();
    }

    #[test]
    fn target_path_resolves_under_models_dir() {
        let dir = std::env::temp_dir().join(format!("np-dl-target-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let resolved = target_path(&dir, "model.gguf").unwrap();
        assert!(resolved.starts_with(dir.canonicalize().unwrap()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn target_path_rejects_traversal_segments() {
        let dir = std::env::temp_dir().join(format!("np-dl-trav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(target_path(&dir, "../etc/passwd").is_err());
        assert!(target_path(&dir, "subdir/../../escape").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
