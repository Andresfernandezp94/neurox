//! Backend factory — builds a `Box<dyn LlmBackend>` from a provider kind.
//!
//! Implemented kinds: `Minimax` (EP-0009-01), `OpenaiCompat` (EP-0009-02),
//! `Anthropic` (EP-0009-03).

use std::sync::Arc;

use super::anthropic::AnthropicBackend;
use super::minimax::MiniMaxBackend;
use super::openai_compat::OpenAiCompatBackend;
use super::{LlmBackend, LlmProviderKind};

/// Build a backend for the given provider kind.
///
/// `api_key` is `None` for providers that do not require auth (e.g. Ollama).
/// `id` is the provider identifier (used by multi-provider kinds as the
/// backend's `provider_id()`); single-kind backends ignore it.
/// `http_client` is the shared `Arc<reqwest::Client>` from `Engine::http_client`.
/// Errors are descriptive: missing key for key-requiring kinds, or not-yet
/// implemented kinds.
pub fn build_backend(
    id: String,
    kind: LlmProviderKind,
    api_key: Option<String>,
    base_url: String,
    model: String,
    http_client: Arc<reqwest::Client>,
) -> Result<Box<dyn LlmBackend>, String> {
    match kind {
        LlmProviderKind::Minimax => {
            let key = api_key.ok_or_else(|| "provider minimax: api_key_env not set".to_string())?;
            Ok(Box::new(MiniMaxBackend::new(
                http_client,
                key,
                base_url,
                model,
            )))
        }
        LlmProviderKind::OpenaiCompat => Ok(Box::new(OpenAiCompatBackend::new(
            http_client,
            id,
            api_key,
            base_url,
            model,
            std::collections::HashMap::new(),
            true,
        ))),
        LlmProviderKind::Anthropic => {
            let key = api_key.ok_or_else(|| "provider anthropic: api_key not set".to_string())?;
            Ok(Box::new(AnthropicBackend::new(
                http_client,
                key,
                base_url,
                model,
            )))
        }
    }
}
