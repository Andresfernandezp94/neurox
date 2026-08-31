//! Streamed events emitted by LLM backends during chat. The daemon
//! subscribes via the per-session SSE channel and relays them to the
//! UI.
//!
//! This is a minimal subset of the daemon's `Event` enum — only the
//! variants that LLM streaming actually emits. The full event
//! vocabulary (session lifecycle, approvals, metrics) lives in the
//! daemon and is added to the SSE stream via the relay layer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Token-level content chunk from the LLM.
    Content {
        session_id: Uuid,
        text: String,
    },
    /// Model "thinking" trace (e.g. Claude/Anthropic, DeepSeek R1).
    Thinking {
        session_id: Uuid,
        text: String,
    },
    /// LLM requested a tool call — relayed to the daemon for execution.
    ToolCall {
        session_id: Uuid,
        tool: String,
        args: serde_json::Value,
        iteration: u32,
    },
    /// Final response from the LLM — ends the turn.
    Done {
        session_id: Uuid,
        text: String,
    },
    /// Error from the LLM or the streaming pipeline.
    Error {
        session_id: Option<Uuid>,
        message: String,
    },
}
