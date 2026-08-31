//! LLM client — talks to the upstream provider (OpenAI-compatible API).
//!
//! 🟡 STUB. Copy the implementation from `agents/agent/src/llm.rs` and
//!    adjust the `base_url` defaults for your provider.
//!
//! ## What this module is for
//!
//! Wraps the LLM HTTP API. Two main responsibilities:
//!   1. `chat_stream` — streaming chat that emits `content_delta`
//!      JSON-RPC notifications as tokens arrive.
//!   2. `chat_utility` — non-streaming single-shot for things like
//!      memory compaction summaries.
//!
//! Real implementations should:
//!   - Add retry with exponential backoff (see EP-0003).
//!   - Honor `Retry-After` header on 429/5xx.
//!   - Map HTTP errors to a typed `AgentError`.
//!   - Strip `<think>...</think>` blocks from the final text.
//!   - Parse `tool_calls` deltas from the SSE stream.

#[allow(dead_code)]
pub struct LlmClient {
    pub base_url: String,
    pub model: String,
}

#[allow(dead_code)]
impl LlmClient {
    pub fn new(_api_key: String, base_url: String, model: String) -> Self {
        Self { base_url, model }
    }
}
