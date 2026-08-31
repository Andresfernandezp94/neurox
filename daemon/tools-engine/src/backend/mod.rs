//! LLM backends — provider abstraction for the LLM agent.
//!
//! `LlmBackend` is a trait implemented per provider. Each backend emits
//! `StreamEvent::Content` / `StreamEvent::Thinking` to a broadcast channel
//! instead of writing JSON-RPC notifications to stdout.

pub mod anthropic;
pub mod chat;
pub mod event;
pub mod factory;
pub mod minimax;
pub mod openai_compat;

pub use chat::ChatMessage;
pub use event::StreamEvent;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Supported LLM provider kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProviderKind {
    Minimax,
    OpenaiCompat,
    Anthropic,
}

impl LlmProviderKind {
    /// Parse from a config string (lowercase). Unknown kinds are rejected.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "minimax" => Some(Self::Minimax),
            "openai_compat" | "openai" => Some(Self::OpenaiCompat),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    /// String identifier for this kind (inverse of `from_str`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimax => "minimax",
            Self::OpenaiCompat => "openai_compat",
            Self::Anthropic => "anthropic",
        }
    }
}

// ─── Public types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

pub struct LlmResult {
    pub text: String,
    pub thinking: Option<String>,
    pub tool_calls: Option<Vec<ToolCallRequest>>,
    pub tokens_out: usize,
}

/// Abstraction over LLM providers. Implemented per backend (MiniMax, OpenAI
/// compatible, Anthropic). The default module only depends on this trait.
///
/// NOTE (EP-0009-01): the design originally specified RPITIT natively, but
/// async fn in traits is NOT object-safe, so `Box<dyn LlmBackend>` requires
/// `async_trait`. It is already a workspace dependency.
#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync {
    /// Stable provider identifier (e.g. `"minimax"`, `"openrouter"`).
    fn provider_id(&self) -> &str;
    /// Active model identifier.
    fn model(&self) -> &str;
    /// Streaming chat call — emits `StreamEvent::Content` / `StreamEvent::Thinking`
    /// per chunk.
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolSpec>,
        session_id: Uuid,
        event_tx: &broadcast::Sender<StreamEvent>,
        cancel: &CancellationToken,
    ) -> Result<LlmResult, String>;
    /// Non-streaming utility call (e.g. compaction summaries).
    async fn chat_utility(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> String;
}

// ─── Conversion helpers ─────────────────────────────────────────────────────

/// Convert core ToolSpecs to default ToolSpecs for the LLM API format.
pub fn core_tools_to_llm(tools: &[crate::tools::ToolSpec]) -> Vec<ToolSpec> {
    tools
        .iter()
        .map(|t| ToolSpec {
            kind: "function".into(),
            function: ToolFunction {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            },
        })
        .collect()
}

/// Strip `<think>...</think>` tags from content, returning (thinking, clean_text).
pub fn strip_thinking(text: &str) -> (Option<String>, String) {
    if let Some(start) = text.find("<think>") {
        if let Some(end) = text.find("</think>") {
            let thinking = text[start + 7..end].trim().to_string();
            let mut clean = String::new();
            clean.push_str(&text[..start]);
            clean.push_str(&text[end + 8..]);
            let clean = clean.trim().to_string();
            return (
                if thinking.is_empty() {
                    None
                } else {
                    Some(thinking)
                },
                clean,
            );
        }
    }
    (None, text.to_string())
}
