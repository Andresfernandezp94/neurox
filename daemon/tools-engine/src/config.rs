//! Configuration types for the engine.
//!
//! The engine doesn't have its own CLI args or TOML config file (it lives
//! inside the daemon process now). The only config it owns is the
//! `LlmProviderConfig` shape that flows through the SQLite store — and
//! that shape is the source of truth for the `/v1/llm/providers/*` HTTP
//! surface.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Provider backend kind. F5.1: serialized on the wire so the
/// daemon's `LlmProviderKind` round-trips correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderKind {
    Minimax,
    OpenaiCompat,
    Anthropic,
}

impl LlmProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmProviderKind::Minimax => "minimax",
            LlmProviderKind::OpenaiCompat => "openai_compat",
            LlmProviderKind::Anthropic => "anthropic",
        }
    }

    /// Parse from a wire string (`"minimax"`, `"openai_compat"`,
    /// `"anthropic"`). Returns `None` for unknown values.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "minimax" => Some(Self::Minimax),
            "openai_compat" | "openai" => Some(Self::OpenaiCompat),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
}

/// A single LLM provider declaration. F5.1: matches the
/// daemon's `core::config::LlmProviderConfig` shape so the daemon
/// can keep storing rows in its SQLite and the engine can read/write
/// the same wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    /// Unique provider id.
    pub id: String,
    pub kind: LlmProviderKind,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    /// Name of the env var holding the API key. The key value is NEVER
    /// stored in config — only the env var name.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, String>,
    /// EP-0018: local service orchestration. `Some` ⇒ the daemon starts
    /// and supervises this provider's process (e.g. llama-server).
    #[serde(default)]
    pub local_command: Option<String>,
    #[serde(default)]
    pub local_args: Vec<String>,
    /// Path to the local model file (GGUF). Tilde is expanded at spawn time.
    #[serde(default)]
    pub local_model_path: Option<String>,
    /// Port the local service listens on.
    #[serde(default)]
    pub local_port: Option<u16>,
}

impl LlmProviderConfig {
    pub fn effective_base_url(&self) -> String {
        if !self.base_url.is_empty() {
            self.base_url.clone()
        } else if let Some(port) = self.local_port {
            format!("http://127.0.0.1:{port}/v1")
        } else {
            match self.kind {
                LlmProviderKind::Minimax => "https://api.minimaxi.chat/v1".to_string(),
                LlmProviderKind::OpenaiCompat => "https://api.openai.com/v1".to_string(),
                LlmProviderKind::Anthropic => "https://api.anthropic.com".to_string(),
            }
        }
    }

    pub fn effective_model(&self) -> String {
        if !self.model.is_empty() {
            self.model.clone()
        } else {
            match self.kind {
                LlmProviderKind::Minimax => "MiniMax-M3".to_string(),
                LlmProviderKind::OpenaiCompat => "gpt-4o-mini".to_string(),
                LlmProviderKind::Anthropic => "claude-3-5-haiku-latest".to_string(),
            }
        }
    }
}

/// EP-0004 wave 1: bundle of providers + default — what the agent
/// subprocess needs to bootstrap its `chat_stream`. Built by
/// `Engine::load_llm_config` from the providers table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider id used when an agent specifies no provider.
    #[serde(default = "default_default_provider")]
    pub default_provider: String,
    /// Explicit default model id (e.g. `"mistral-medium-latest"`).
    /// When set, overrides the chosen provider's `effective_model()`.
    /// `None` (missing) → use provider's configured default model.
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub providers: Vec<LlmProviderConfig>,
}

fn default_default_provider() -> String {
    "minimax".to_string()
}
