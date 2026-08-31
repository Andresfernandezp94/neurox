//! Discovery of available models from LLM providers.
//!
//! Each provider kind has its own models endpoint format:
//! - OpenAI-compatible: `GET <base_url>/models` → `{ data: [{ id }] }`
//! - Anthropic: `GET <base_url>/v1/models` with `anthropic-version` header
//! - MiniMax: Same as OpenAI-compat format
//!
//! Capability classification uses a heuristic on the model id (no extra
//! network call). Results are cached in memory for `CAPABILITY_TTL_SECS`
//! to avoid recomputing on every list call.

use crate::config::{LlmProviderConfig, LlmProviderKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Coarse capability bucket for a model. The classification is heuristic
/// (model id substring match); providers don't expose a uniform field for
/// this in the standard `/models` endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelCapability {
    TextGeneration,
    Chat,
    Embeddings,
    Image,
    Vision,
    Audio,
    Tts,
    Transcription,
    Code,
    Rerank,
    #[default]
    Unknown,
}

impl ModelCapability {
    /// Human-readable label for UI rendering (Title Case).
    pub fn label(self) -> &'static str {
        match self {
            ModelCapability::TextGeneration => "Text Generation",
            ModelCapability::Chat => "Chat",
            ModelCapability::Embeddings => "Embeddings",
            ModelCapability::Image => "Image Gen",
            ModelCapability::Vision => "Vision",
            ModelCapability::Audio => "Audio",
            ModelCapability::Tts => "TTS",
            ModelCapability::Transcription => "Transcription",
            ModelCapability::Code => "Code",
            ModelCapability::Rerank => "Rerank",
            ModelCapability::Unknown => "Unknown",
        }
    }
}

/// Heuristic capability classifier. The check is order-sensitive:
/// more specific keywords win over general ones (e.g. "vision" beats
/// "gpt" so a `gpt-4-vision` model is classified as Vision, not
/// TextGeneration).
pub fn capability_for_model_id(model_id: &str) -> ModelCapability {
    let id = model_id.to_ascii_lowercase();
    // Embeddings come first: the word "embed" is highly specific.
    if id.contains("embed") {
        return ModelCapability::Embeddings;
    }
    // TTS and transcription are highly specific too.
    if id.contains("tts") || id.contains("text-to-speech") {
        return ModelCapability::Tts;
    }
    if id.contains("whisper") || id.contains("transcription") {
        return ModelCapability::Transcription;
    }
    if id.contains("rerank") {
        return ModelCapability::Rerank;
    }
    // Image generation. Match the keyword as a whole token — the model
    // id is normalised to lower-case and split on `-`, `:`, `_`, `/`,
    // and `.`. This avoids "image" inside "image-style" (rare) without
    // missing "gemini-3-pro-image" or "image-1".
    {
        let tokens: Vec<&str> = id.split(['-', ':', '_', '/', '.']).collect();
        if tokens.iter().any(|t| {
            *t == "image"
                || *t == "imagen"
                || *t == "dall"
                || *t == "dalle"
                || *t == "sdxl"
                || *t == "midjourney"
        }) {
            return ModelCapability::Image;
        }
    }
    // Vision / multimodal.
    if id.contains("vision") || id.contains("-vl-") || id.contains("gpt-4v") {
        return ModelCapability::Vision;
    }
    // Audio (real-time or general audio, not TTS).
    if id.contains("audio") || id.contains("realtime") {
        return ModelCapability::Audio;
    }
    // Code-specific.
    if id.contains("codex")
        || id.contains("code-")
        || id.contains("coder")
        || id.contains("codestral")
    {
        return ModelCapability::Code;
    }
    // Chat-tuned.
    if id.contains("instruct") || id.contains("chat") {
        return ModelCapability::Chat;
    }
    // Heuristic didn't match — fall back to text generation, the most
    // common bucket.
    ModelCapability::TextGeneration
}

/// A discovered model from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredModel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
    /// Heuristic capability bucket (cached, see [`capability_for_model_id`]).
    #[serde(default)]
    pub capability: ModelCapability,
    /// Heuristic: model id matches a known list of tool-supporting models.
    /// Surfaced as a "tools" tag in the UI so operators can filter.
    #[serde(default)]
    pub supports_tools: bool,
    /// Heuristic: model id contains "free" (case-insensitive). Surfaces the
    /// free-tier models from OpenRouter (`:free` suffix) and similar.
    #[serde(default)]
    pub is_free: bool,
}

/// Heuristic: does this model id match a known family of tool-supporting
/// models? This is a coarse approximation — providers don't expose a uniform
/// `tools` field in `/models` responses, so we match known prefixes.
fn supports_tools_heuristic(model_id: &str) -> bool {
    let id = model_id.to_lowercase();
    const FAMILIES: &[&str] = &[
        "gpt-4", "gpt-5", "o1", "o3", "o4",
        "claude-3", "claude-4",
        "minimax", "abab",
        "qwen", "qwq",
        "llama-3.1", "llama-3.2", "llama-3.3", "llama-4",
        "mistral", "mixtral",
        "deepseek", "gemini", "command",
    ];
    FAMILIES.iter().any(|f| id.contains(f))
}

/// Discover models from a provider. Returns an empty vec on failure
/// (timeout, auth error, etc.) — never errors to the caller.
pub async fn discover_models(
    provider: &LlmProviderConfig,
    api_key: Option<&str>,
) -> Vec<DiscoveredModel> {
    let base_url = provider.effective_base_url();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    match provider.kind {
        LlmProviderKind::Minimax | LlmProviderKind::OpenaiCompat => {
            discover_openai_compat(&client, &base_url, api_key).await
        }
        LlmProviderKind::Anthropic => discover_anthropic(&client, &base_url, api_key).await,
    }
}

/// OpenAI-compatible models endpoint: `GET <base_url>/models`
async fn discover_openai_compat(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Vec<DiscoveredModel> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = client.get(&url);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(url = %url, error = %e, "models discovery failed (openai_compat)");
            return Vec::new();
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(
            url = %url,
            status = %resp.status(),
            "models discovery returned non-200 (openai_compat)"
        );
        return Vec::new();
    }

    #[derive(Deserialize)]
    struct ModelsResponse {
        data: Option<Vec<ModelEntry>>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        owned_by: Option<String>,
    }

    match resp.json::<ModelsResponse>().await {
        Ok(body) => {
            let entries: Vec<ModelEntry> = body.data.unwrap_or_default();
            entries
                .into_iter()
                .map(|m| {
                    let id = m.id.clone();
                    let id_lower = id.to_lowercase();
                    let capability = capability_cached(&id);
                    DiscoveredModel {
                        id,
                        owned_by: m.owned_by,
                        capability,
                        supports_tools: supports_tools_heuristic(&id_lower),
                        is_free: id_lower.contains("free"),
                    }
                })
                .collect()
        }
        Err(e) => {
            tracing::debug!(url = %url, error = %e, "failed to parse models response");
            Vec::new()
        }
    }
}

/// Anthropic models endpoint: `GET <base_url>/v1/models`
async fn discover_anthropic(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Vec<DiscoveredModel> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let mut req = client.get(&url).header("anthropic-version", "2023-06-01");
    if let Some(key) = api_key {
        req = req.header("x-api-key", key);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(url = %url, error = %e, "models discovery failed (anthropic)");
            return Vec::new();
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(
            url = %url,
            status = %resp.status(),
            "models discovery returned non-200 (anthropic)"
        );
        return Vec::new();
    }

    // Anthropic returns { data: [{ id, ... }] } similar to OpenAI
    #[derive(Deserialize)]
    struct AnthropicModelsResponse {
        data: Option<Vec<AnthropicModelEntry>>,
    }
    #[derive(Deserialize)]
    struct AnthropicModelEntry {
        id: String,
    }

    match resp.json::<AnthropicModelsResponse>().await {
        Ok(body) => {
            let entries: Vec<AnthropicModelEntry> = body.data.unwrap_or_default();
            entries
                .into_iter()
                .map(|m| {
                    let id = m.id.clone();
                    let id_lower = id.to_lowercase();
                    let capability = capability_cached(&id);
                    DiscoveredModel {
                        id,
                        owned_by: Some("anthropic".to_string()),
                        capability,
                        supports_tools: supports_tools_heuristic(&id_lower),
                        is_free: id_lower.contains("free"),
                    }
                })
                .collect()
        }
        Err(e) => {
            tracing::debug!(url = %url, error = %e, "failed to parse anthropic models response");
            Vec::new()
        }
    }
}

// ─── Capability cache ────────────────────────────────────────────────────
//
// Heuristic classification is pure CPU work but a panel might list the
// same provider many times per minute. A simple TTL map avoids the
// re-match overhead and stabilises the per-process footprint. The map
// is bounded to MAX_ENTRIES (FIFO eviction once full) so a long-running
// daemon can't grow it unboundedly across many providers.

const CAPABILITY_TTL_SECS: u64 = 300;
const CAPABILITY_MAX_ENTRIES: usize = 4096;

#[derive(Debug, Clone)]
struct CacheEntry {
    capability: ModelCapability,
    inserted_at: Instant,
}

static CAPABILITY_CACHE: std::sync::OnceLock<Mutex<HashMap<String, CacheEntry>>> =
    std::sync::OnceLock::new();

fn capability_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    CAPABILITY_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Return the cached capability for `model_id`, computing and storing it
/// if absent or expired.
pub fn capability_cached(model_id: &str) -> ModelCapability {
    let now = Instant::now();
    let ttl = Duration::from_secs(CAPABILITY_TTL_SECS);

    // Fast path: read-only access under the lock. We hold the lock
    // briefly and release before doing any work that could re-enter.
    {
        let cache = capability_cache()
            .lock()
            .expect("capability cache poisoned");
        if let Some(entry) = cache.get(model_id) {
            if now.duration_since(entry.inserted_at) < ttl {
                return entry.capability;
            }
        }
    }

    // Slow path: compute, then store. We allow the (rare) race where two
    // callers compute the same value — the entry is overwritten with the
    // same result, no harm done.
    let capability = capability_for_model_id(model_id);

    let mut cache = capability_cache()
        .lock()
        .expect("capability cache poisoned");
    if cache.len() >= CAPABILITY_MAX_ENTRIES {
        // FIFO eviction: drop the oldest entry. We do a single scan
        // because the map is small (≤ 4096) and the operation is rare.
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, v)| v.inserted_at)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&oldest_key);
        }
    }
    cache.insert(
        model_id.to_string(),
        CacheEntry {
            capability,
            inserted_at: now,
        },
    );

    capability
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_heuristic_examples() {
        // Embeddings.
        assert_eq!(
            capability_for_model_id("text-embedding-3-small"),
            ModelCapability::Embeddings
        );
        assert_eq!(
            capability_for_model_id("text-embedding-ada-002"),
            ModelCapability::Embeddings
        );
        // TTS.
        assert_eq!(capability_for_model_id("tts-1"), ModelCapability::Tts);
        assert_eq!(
            capability_for_model_id("gpt-4o-mini-tts"),
            ModelCapability::Tts
        );
        // Vision wins over chat.
        assert_eq!(
            capability_for_model_id("gpt-4-vision-preview"),
            ModelCapability::Vision
        );
        assert_eq!(
            capability_for_model_id("claude-3-5-sonnet-vision"),
            ModelCapability::Vision
        );
        // Code.
        assert_eq!(
            capability_for_model_id("deepseek-coder"),
            ModelCapability::Code
        );
        assert_eq!(
            capability_for_model_id("codestral-22b"),
            ModelCapability::Code
        );
        // Chat.
        assert_eq!(
            capability_for_model_id("open-mistral-7b"),
            ModelCapability::TextGeneration
        );
        assert_eq!(
            capability_for_model_id("llama-3-8b-instruct"),
            ModelCapability::Chat
        );
        // Default.
        assert_eq!(
            capability_for_model_id("gpt-4o"),
            ModelCapability::TextGeneration
        );
        // Audio.
        assert_eq!(
            capability_for_model_id("gpt-4o-realtime-preview"),
            ModelCapability::Audio
        );
        // Image.
        assert_eq!(capability_for_model_id("dall-e-3"), ModelCapability::Image);
        // Whisper.
        assert_eq!(
            capability_for_model_id("whisper-1"),
            ModelCapability::Transcription
        );
    }

    #[test]
    fn capability_cached_returns_same_value_twice() {
        // First call: cache miss → computes + stores.
        let c1 = capability_cached("gpt-4o");
        // Second call: cache hit → returns the stored value.
        let c2 = capability_cached("gpt-4o");
        assert_eq!(c1, c2);
    }
}
