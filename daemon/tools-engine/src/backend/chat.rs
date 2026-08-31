//! Chat message format — the unified representation used by every
//! LLM backend in llmd. Mirrors the OpenAI chat-completions shape so
//! the same messages can be sent to any provider after light
//! translation.
//!
//! Distinct from `crate::tools::ToolSpec` (the tool catalogue) —
//! `ChatMessage` is the per-turn conversation entry, while `ToolSpec`
//! describes what tools the LLM can call.

use serde::{Deserialize, Serialize};

use super::ToolCallRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRequest>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
