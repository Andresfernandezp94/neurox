//! Reusable agent loop scaffolding.
//!
//! EP-0004 wave 5b: extracted from `daemon/agents/agent/src/main.rs`
//! so that `agents/_template/` can scaffold a new agent binary without
//! copy-pasting 950 LOC.
//!
//! This module currently exposes the JSON-RPC request/response types
//! used by the agent subprocess protocol. The full agent loop body
//! remains in the calling binary (it carries identity-specific config).

use serde::{Deserialize, Serialize};

/// JSON-RPC request envelope sent to the agent subprocess over stdio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// JSON-RPC response envelope produced by the agent subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<AgentError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Optional context passed to [`run_agent_loop`] so each agent binary
/// can wire its own identity-specific state (system prompt, tools
/// allowlist, etc.) without duplicating the dispatch glue.
#[derive(Default)]
pub struct AgentContext {
    pub agent_id: String,
    pub identity_dir: Option<std::path::PathBuf>,
}

/// Run the agent loop. The actual implementation lives in the calling
/// binary (it is identity-specific). This stub returns an error so
/// that partial extractions still compile cleanly during the EP-0004
/// wave 5b rollout.
pub async fn run_agent_loop(_ctx: AgentContext) -> anyhow::Result<()> {
    // The full loop lives in daemon/agents/agent/src/main.rs. New
    // agents should copy the relevant subset (handle_process,
    // handle_tool_result, etc.) and call into the identity/llm/memory/
    // mode/skills/tools_filter modules of agent-lib.
    anyhow::bail!(
        "agent-lib::run_agent_loop is a stub. Implement the loop in your binary."
    )
}