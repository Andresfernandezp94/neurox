//! Working memory — the agent's short-term message history.
//!
//! 🟡 STUB. Copy the implementation from `agents/agent/src/memory.rs`.
//!
//! ## What this module is for
//!
//! Holds the list of `ChatMessage`s the agent will send to the LLM on
//! the next `process` call. Responsibilities:
//!   1. Append user / assistant / tool messages in order.
//!   2. Track token count (rough estimate: chars / 4).
//!   3. Trigger compaction when the count exceeds a threshold —
//!      summarize older messages via `LlmClient::chat_utility`.
//!   4. Expose `get_messages_for_llm()` shaped as the provider expects.

#[allow(dead_code)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[allow(dead_code)]
    pub tool_calls: Option<Vec<String>>,
    #[allow(dead_code)]
    pub tool_call_id: Option<String>,
}

#[allow(dead_code)]
pub struct WorkingMemory {
    messages: Vec<ChatMessage>,
}

#[allow(dead_code)]
impl WorkingMemory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
