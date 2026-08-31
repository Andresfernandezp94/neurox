//! Hugging Face model discovery (EP-0018-07).
//!
//! Calls HF's public Models API to find GGUF quantisations. We do NOT
//! require an API token for the public search; gated models return 403
//! and we surface that as a per-result hint rather than a hard failure.
//!
//! Endpoint: `GET https://huggingface.co/api/models?search=<q>&filter=gguf`
//!
//! Response shape (simplified):
//! ```json
//! {
//!   "models": [
//!     {"id": "owner/repo", "author": "owner", "downloads": 1234,
//!      "lastModified": "2024-...", "tags": ["gguf", ...], "gated": false}
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;

const HF_API_BASE: &str = "https://huggingface.co/api/models";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESULTS: usize = 30;

/// Single result row returned by `search`.
#[derive(Debug, Clone, Serialize)]
pub struct HfModel {
    /// Repo id like `owner/name`.
    pub id: String,
    pub author: String,
    pub downloads: u64,
    pub last_modified: Option<String>,
    /// True if this model has at least one file tagged `gguf`.
    pub has_gguf: bool,
    pub gated: bool,
}

impl HfModel {
    /// Human-friendly display name (last path segment).
    pub fn display_name(&self) -> &str {
        self.id.rsplit('/').next().unwrap_or(&self.id)
    }
}

/// Raw row from the HF API. We only deserialize what we use.
#[derive(Debug, Deserialize)]
struct HfRaw {
    #[serde(rename = "modelId")]
    id: Option<String>,
    id_fallback: Option<String>,
    author: Option<String>,
    downloads: Option<u64>,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    tags: Option<Vec<String>>,
    gated: Option<bool>,
}

impl HfRaw {
    fn into_model(self) -> Option<HfModel> {
        let id = self.id.or(self.id_fallback)?;
        let tags = self.tags.unwrap_or_default();
        Some(HfModel {
            id,
            author: self.author.unwrap_or_default(),
            downloads: self.downloads.unwrap_or(0),
            last_modified: self.last_modified,
            has_gguf: tags.iter().any(|t| t.eq_ignore_ascii_case("gguf")),
            gated: self.gated.unwrap_or(false),
        })
    }
}

/// Search Hugging Face for models matching `query`. Filters out results
/// without a `gguf` tag. Returns up to `MAX_RESULTS` rows.
pub async fn search(query: &str) -> Result<Vec<HfModel>, String> {
    if query.trim().is_empty() {
        return Err("search query must not be empty".into());
    }
    let url = format!(
        "{HF_API_BASE}?search={}&filter=gguf&limit={}&direction=-1&sort=downloads",
        url_encode(query),
        MAX_RESULTS,
    );

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("neurox/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("failed to build HF client: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HF request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HF returned HTTP {}", resp.status()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| format!("failed to read HF response: {e}"))?;

    let raw: Vec<HfRaw> =
        serde_json::from_str(&body).map_err(|e| format!("HF response is not JSON: {e}"))?;

    Ok(raw.into_iter().filter_map(HfRaw::into_model).collect())
}

/// URL-encode a query string for the HF API (preserves reserved chars
/// the HF search endpoint accepts).
pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Test-only seam: re-export the raw `HfRaw` parser so unit tests can
/// exercise the filter logic without hitting the network.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn parse_raw_response(body: &str) -> Result<Vec<HfModel>, String> {
    let raw: Vec<HfRaw> =
        serde_json::from_str(body).map_err(|e| format!("HF response is not JSON: {e}"))?;
    Ok(raw.into_iter().filter_map(HfRaw::into_model).collect())
}
