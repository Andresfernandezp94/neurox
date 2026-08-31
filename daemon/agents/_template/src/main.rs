//! Agent template — minimal JSON-RPC scaffolding for a neurox agent.
//!
//! This is the **skeleton** every neurox agent starts from. Copy the
//! `_template/` folder, rename the crate, replace the stubs in the
//! `mod` blocks below, and you'll have a fully-fledged agent that the
//! daemon can spawn.
//!
//! ## How to bootstrap a new agent
//!
//! ```bash
//! # 1. Copy the template
//! cp -r agents/_template agents/my_agent
//!
//! # 2. Rename the crate
//! sed -i 's/agent-template/my-agent/g; s/_template/my_agent/g' agents/my_agent/Cargo.toml
//! mv agents/my_agent/src/{_template,my_agent}.rs   # optional
//!
//! # 3. Register in the workspace
//! # Add "agents/my_agent" to daemon/Cargo.toml's [workspace].members
//!
//! # 4. Fill in the modules (see TODOs in each)
//! $EDITOR agents/my_agent/src/{identity,llm,memory,mode,skills,tools_filter}.rs
//!
//! # 5. Register the agent in daemon/core/src/config/mod.rs
//! # Add a new entry to `default_agent_specs()` with id = "my_agent"
//! ```
//!
//! ## What this template gives you
//!
//! - ✅ JSON-RPC 2.0 loop over stdin/stdout
//! - ✅ `ping` and `reset_session` methods (working out of the box)
//! - ✅ Graceful error handling with `-32603` codes
//! - ✅ Structured logging via `tracing`
//! - 🟡 `process` and `tool_result` are stubs — see `TODO` markers
//! - 🟡 The six sibling modules (`identity`, `llm`, `memory`, `mode`,
//!   `skills`, `tools_filter`) are NOT yet wired in — add them as
//!   needed in `handle_process()` / `handle_tool_result()` below.
//!
//! ## Wire format reminder
//!
//! - **Inbound** (daemon → you, one JSON object per line):
//!   ```json
//!   {"jsonrpc":"2.0","id":1,"method":"process","params":{"text":"..."}}
//!   {"jsonrpc":"2.0","id":2,"method":"tool_result","params":{"call_id":"...","result":"..."}}
//!   {"jsonrpc":"2.0","id":3,"method":"ping"}
//!   {"jsonrpc":"2.0","id":4,"method":"reset_session"}
//!   ```
//!
//! - **Outbound** (you → daemon):
//!   - Response: `{"jsonrpc":"2.0","id":1,"result":{...}}` or `{"error":{...}}`
//!   - Notification: `{"jsonrpc":"2.0","method":"content_delta","params":{"text":"..."}}`

mod identity;
mod llm;
mod memory;
mod mode;
mod skills;
mod tools_filter;

use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

/// State the agent holds between JSON-RPC requests.
///
/// The template ships with the minimum fields needed to compile.
/// Real agents (like `agent`) split this into `MutableState` (under
/// `RwLock`) and `ColdState` (under `Arc`) so streaming LLM calls
/// don't block other sessions — see EP-0003 for the design.
struct AgentState {
    /// Bump this on `reset_session`.
    session_id: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    eprintln!("[agent_template] starting (replace this agent with your own)");

    let state = Arc::new(Mutex::new(AgentState { session_id: 0 }));
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let stdout: Arc<Mutex<tokio::io::Stdout>> =
        Arc::new(Mutex::new(tokio::io::stdout()));

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF — daemon closed our stdin
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                write_error(&stdout, None, -32700, &format!("parse: {e}")).await?;
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        let result = handle_method(&method, params, state.clone()).await;

        let out = match result {
            Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
            Err(msg) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32603, "message": msg},
            }),
        };
        {
            let mut w = stdout.lock().await;
            w.write_all(format!("{out}\n").as_bytes()).await?;
            w.flush().await?;
        }
    }
    Ok(())
}

async fn handle_method(
    method: &str,
    params: Value,
    state: Arc<Mutex<AgentState>>,
) -> Result<Value, String> {
    eprintln!("[agent_template] method={method}");

    match method {
        "ping" => {
            // ✅ Working: returns immediately. Replace with a richer
            //    payload (model name, token usage, etc.) once you wire
            //    up `llm::LlmClient`.
            let s = state.lock().await;
            Ok(json!({
                "pong": true,
                "session_id": s.session_id,
                "agent": "agent-template",
            }))
        }

        "reset_session" => {
            // ✅ Working: bumps the session_id. Real agents should also
            //    clear their working memory and reload identity facts.
            let mut s = state.lock().await;
            s.session_id += 1;
            eprintln!(
                "[agent_template] session reset, new session_id={}",
                s.session_id
            );
            Ok(json!({"reset": true, "session_id": s.session_id}))
        }

        "process" => {
            // 🟡 TODO: implement your agent loop here.
            //
            //   1. Extract `params.text`, `params.tools`, `params.session_id`.
            //   2. Load identity (see `identity.rs`).
            //   3. Detect mode (Chat/Plan/Build — see `mode.rs`).
            //   4. Match skills against the text (see `skills.rs`).
            //   5. Filter tools by mode (see `tools_filter.rs`).
            //   6. Add the user message to working memory (see `memory.rs`).
            //   7. Build the system prompt (see `identity::build_system_prompt`).
            //   8. Call the LLM (see `llm::LlmClient::chat_stream`).
            //   9. Emit `content_delta` notifications as tokens arrive.
            //  10. Return either `{"text": "...", "tool_call": {...}}` or
            //      `{"text": "..."}` depending on whether the LLM wants
            //      to call a tool.
            //
            //   See `agents/agent/src/main.rs` for a complete reference.
            Err("process: not implemented yet — see TODO in src/main.rs".into())
        }

        "tool_result" => {
            // Avoid unused warning for `params` until you wire it up.
            let _ = params;
            // 🟡 TODO: handle a tool execution result returned by the daemon.
            //
            //   1. Extract `params.call_id` and `params.result`.
            //   2. Append a `tool` role message to working memory
            //      (memory::ChatMessage { role: "tool", tool_call_id,
            //      content: result }).
            //   3. Re-call the LLM with the updated history so it can
            //      continue (usually with `Mode::Build` to give full
            //      tool access for the follow-up).
            //   4. Return the next assistant message (text and/or
            //      tool_call).
            //
            //   See `agents/agent/src/main.rs::handle_tool_result`.
            Err("tool_result: not implemented yet — see TODO in src/main.rs".into())
        }

        other => Err(format!("unknown method: {other}")),
    }
}

async fn write_error(
    stdout: &Arc<Mutex<tokio::io::Stdout>>,
    id: Option<Value>,
    code: i32,
    message: &str,
) -> anyhow::Result<()> {
    let out = json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {"code": code, "message": message},
    });
    let mut w = stdout.lock().await;
    w.write_all(format!("{out}\n").as_bytes()).await?;
    w.flush().await?;
    Ok(())
}
