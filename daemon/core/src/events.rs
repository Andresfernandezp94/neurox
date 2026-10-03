use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::approval::ApprovalRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    SessionStarted {
        session_id: Uuid,
        agent_id: String,
    },
    SessionEnded {
        session_id: Uuid,
        summary: Option<String>,
    },
    Thinking {
        session_id: Uuid,
        text: String,
        /// EP-2026-09-05 (stream seq): strictly-increasing per-session
        /// sequence number stamped at emission to the bus. Lets the
        /// client apply each stream chunk exactly once (ordered by
        /// seq) even though it may arrive via two paths (SSE + WS).
        #[serde(default)]
        seq: u64,
    },
    Content {
        session_id: Uuid,
        text: String,
        /// EP-2026-09-05 (stream seq): see `Thinking::seq`.
        #[serde(default)]
        seq: u64,
    },
    /// EP-2026-09-05 (cross-device sync): emitted after a user OR
    /// assistant message is persisted. Other devices logged in as
    /// the same user use this to mirror the message into their chat
    /// tab in realtime — without it, the user's own message only
    /// shows on the device that sent it (assistant responses DO
    /// propagate via the stream events, but the user's prompt is
    /// only persisted, not broadcast).
    MessageAppended {
        session_id: Uuid,
        /// i64 id from the messages table. Lets the receiver dedupe
        /// (ignore if the row is already in the local tab's messages).
        message_id: i64,
        role: String,
        content: String,
        /// EP-0026-rev-fix: assistant-only thinking block (the
        /// `<think>…</think>` portion). `None` for user messages
        /// or old rows persisted before the column was added.
        thinking: Option<String>,
        ts: String,
    },
    ToolCall {
        session_id: Uuid,
        tool: String,
        args: serde_json::Value,
        iteration: u32,
        /// EP-2026-10-03: identificador de ESTA llamada. Es el `id` que
        /// genero el modelo y llega intacto desde el agente.
        ///
        /// Antes no existia y el cliente no podia emparejar una llamada
        /// con su resultado: seatia por posicion, lo cual se rompe en
        /// cuanto el modelo pide dos tools iguales. Con `iteration` tampoco
        /// vale: todas las tools de un lote comparten numero de iteracion.
        /// Va en ambos eventos para que el par se pueda cerrar por id.
        call_id: String,
        /// EP-2026-09-05 (stream seq): see `Thinking::seq`.
        #[serde(default)]
        seq: u64,
    },
    ToolResult {
        session_id: Uuid,
        tool: String,
        result: String,
        iteration: u32,
        /// EP-2026-10-03: ver `ToolCall::call_id`. Mismo valor que el de
        /// la llamada a la que responde este resultado.
        call_id: String,
        /// EP-2026-09-05 (stream seq): see `Thinking::seq`.
        #[serde(default)]
        seq: u64,
    },
    AgentSpawned {
        session_id: Uuid,
        agent_id: String,
        ephemeral_id: String,
    },
    AgentFinished {
        session_id: Uuid,
        ephemeral_id: String,
        status: String,
        elapsed_ms: u64,
    },
    Metrics {
        session_id: Uuid,
        iteration: u32,
        tokens_total: u64,
        elapsed_ms: u64,
    },
    Done {
        session_id: Uuid,
        text: String,
    },
    Error {
        session_id: Option<Uuid>,
        message: String,
    },
    /// Emitted when an agent invokes a tool that requires approval.
    /// The user must respond via /v1/approvals/:id/respond or WS command.
    ApprovalRequest {
        request: ApprovalRequest,
    },
    /// Emitted after the user responds to an approval (approve/deny/timeout).
    ApprovalResolved {
        approval_id: Uuid,
        session_id: Uuid,
        tool: String,
        decision: String,
    },
    /// EP-0003 Tier 3: emitted when the in-process agent's compaction
    /// call (the LLM-backed `chat_utility` that summarizes the older
    /// half of the working memory when it gets large) returns an
    /// empty string. The agent continues without the summary, but
    /// the conversation will grow unbounded until the next
    /// successful compaction. The dashboard / frontend can show
    /// this as a warning so the user knows the agent's working
    /// memory is no longer being trimmed.
    CompactionFailed {
        session_id: Uuid,
        reason: String,
    },
}

impl Event {
    #[must_use]
    pub fn session_id(&self) -> Option<Uuid> {
        match self {
            Event::SessionStarted { session_id, .. }
            | Event::SessionEnded { session_id, .. }
            | Event::Thinking { session_id, .. }
            | Event::Content { session_id, .. }
            | Event::MessageAppended { session_id, .. }
            | Event::ToolCall { session_id, .. }
            | Event::ToolResult { session_id, .. }
            | Event::AgentSpawned { session_id, .. }
            | Event::AgentFinished { session_id, .. }
            | Event::Metrics { session_id, .. }
            | Event::Done { session_id, .. }
            | Event::CompactionFailed { session_id, .. }
            | Event::ApprovalResolved { session_id, .. } => Some(*session_id),
            Event::Error { session_id, .. } => *session_id,
            Event::ApprovalRequest { request } => Some(request.session_id),
        }
    }
}
