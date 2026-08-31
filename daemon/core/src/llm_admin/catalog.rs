//! Accumulated model catalog across all configured LLM providers (EP-0017-02).
//!
//! `build_catalog` unions the discovered models of every provider where
//! `configured == true`, deduplicating by `(provider_id, model_id)`.
//! Individual discovery failures are skipped — they never fail the catalog.

use tools_engine::config::{LlmProviderConfig, LlmProviderKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One entry in the accumulated catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub provider_id: String,
    pub model_id: String,
    pub kind: String,
    pub base_url: String,
    /// Heuristic capability bucket (e.g. "chat", "embeddings", "image").
    /// `None` if the field is absent from the upstream discovery payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<tools_engine::providers::models_discovery::ModelCapability>,
    /// Heuristic: model id matches a known tool-supporting family.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub supports_tools: bool,
    /// Heuristic: model id contains "free".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_free: bool,
}

/// A snapshot of the accumulated catalog with its fetch timestamp.
#[derive(Debug, Clone)]
pub struct ModelCatalog {
    pub models: Vec<CatalogEntry>,
    pub fetched_at: DateTime<Utc>,
}

/// Cache TTL for the accumulated catalog (spec 1.4): 5 minutes.
pub const CATALOG_TTL_SECS: i64 = 300;

/// Invalidate the in-memory LLM catalog cache so the next `GET /v1/llm/models`
/// rebuilds it from the current provider set. Called on every mutation that
/// can change the set of configured/active providers (create, update, delete,
/// start, stop) so newly-enabled providers appear in the chat without waiting
/// for the TTL to expire.
pub async fn invalidate_llm_catalog(cache: &tokio::sync::Mutex<Option<ModelCatalog>>) {
    let mut guard = cache.lock().await;
    *guard = None;
}

/// Whether a provider counts as "configured" (has its API key set, or does
/// not require one). Same rule as `GET /v1/llm/providers` (spec 2.3).
pub fn is_configured(provider: &LlmProviderConfig) -> bool {
    match &provider.api_key_env {
        Some(env_name) => std::env::var(env_name).is_ok(),
        None => true, // providers without api_key_env are always configured (e.g. Ollama)
    }
}

/// Snake_case slug for `LlmProviderKind`, consistent with the serde rename
/// used in config and with `http::kind_slug`.
pub fn kind_slug(kind: LlmProviderKind) -> &'static str {
    match kind {
        LlmProviderKind::Minimax => "minimax",
        LlmProviderKind::OpenaiCompat => "openai_compat",
        LlmProviderKind::Anthropic => "anthropic",
    }
}

/// Build the accumulated catalog from the given providers.
///
/// Iterates providers where `is_configured(provider)` is true and calls
/// `discover_models` (reused from EP-0010). Providers whose discovery fails
/// are skipped — the catalog still returns for the rest (spec 1.2).
pub async fn build_catalog(providers: &[LlmProviderConfig]) -> ModelCatalog {
    let mut models: Vec<CatalogEntry> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for provider in providers {
        if !is_configured(provider) {
            continue;
        }
        let api_key = provider
            .api_key_env
            .as_ref()
            .and_then(|env_name| std::env::var(env_name).ok());
        let discovered =
            tools_engine::providers::models_discovery::discover_models(provider, api_key.as_deref())
                .await;
        let base_url = provider.effective_base_url();
        for m in discovered {
            let key = (provider.id.clone(), m.id.clone());
            if seen.insert(key) {
                models.push(CatalogEntry {
                    provider_id: provider.id.clone(),
                    model_id: m.id,
                    kind: kind_slug(provider.kind).to_string(),
                    base_url: base_url.clone(),
                    capability: Some(m.capability),
                    supports_tools: m.supports_tools,
                    is_free: m.is_free,
                });
            }
        }
    }

    // Deterministic ordering: provider_id, then model_id.
    models.sort_by(|a, b| {
        a.provider_id
            .cmp(&b.provider_id)
            .then_with(|| a.model_id.cmp(&b.model_id))
    });

    ModelCatalog {
        models,
        fetched_at: Utc::now(),
    }
}
