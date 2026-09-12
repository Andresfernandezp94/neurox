//! LLM client — delegates to the engine's LlmBackend.
//!
//! EP-2026-08-19 follow-up #6: this used to make a raw HTTP call to
//! `${base_url}/chat/completions` (OpenAI-compat format, MiniMax-specific).
//! Now it builds the engine's `LlmBackend` from the env config and
//! delegates to `backend.chat_stream(...)`. The wire format stays
//! identical (OpenAI-compat SSE), but provider-specific quirks
//! (Anthropic's `system` field, etc.) are now handled in one place
//! in the engine.
//!
//! The agent subprocess doesn't talk to a daemon-internal engine —
//! it has its own copy of `tools-engine` (workspace member) and uses it
//! directly.

use crate::memory::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use uuid::Uuid;


// ─── Public types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunction,
    /// EP-2026-08-19: declarative categories, populated from the
    /// engine's ToolSpec. Used by `tools_filter` to decide relevance
    /// without an LLM round-trip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// EP-2026-08-19: modes the tool is compatible with, populated
    /// from the engine. Empty = "Build only" (legacy default).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mode_compatible: Vec<String>,
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
    pub streamed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmSampling {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub seed: Option<i64>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
}

// ─── LlmClient — wraps the engine's LlmBackend ─────────────────────────────

/// LLM client backed by a `tools_engine::backend::LlmBackend`.
///
/// Constructed from environment variables:
///   - `MINIMAX_API_KEY` (or `NEUROX_LLM_API_KEY`) — required
///   - `MINIMAX_BASE_URL` — defaults to MiniMax
///   - `MINIMAX_MODEL`   — defaults to MiniMax-M3
///   - `NEUROX_LLM_PROVIDER` — `minimax` | `openai_compat` | `anthropic`
///     (defaults to `minimax`)
///
/// The sampling fields are read from the v2 manifest if present;
/// otherwise defaults are applied per backend.
pub struct LlmClient {
    backend: Arc<dyn tools_engine::backend::LlmBackend>,
    sampling: LlmSampling,
}

impl LlmClient {
    /// Build from explicit values (mostly for tests).
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self::with_sampling(api_key, base_url, model, LlmSampling::default())
    }

    /// Build with explicit sampling fields.
    pub fn with_sampling(
        api_key: String,
        base_url: String,
        model: String,
        sampling: LlmSampling,
    ) -> Self {
        // Detect provider kind from env (defaults to MiniMax).
        let kind = std::env::var("NEUROX_LLM_PROVIDER")
            .ok()
            .and_then(|s| tools_engine::backend::LlmProviderKind::from_str(&s))
            .unwrap_or(tools_engine::backend::LlmProviderKind::Minimax);
        Self::with_kind(api_key, base_url, model, kind, sampling)
    }

    /// Build with an explicit provider kind. Used by the agent subprocess
    /// to (re)build its LlmClient on every `process` call when the user
    /// picks a different model — the daemon resolves the provider
    /// config (api_key + base_url + kind) from its config and ships
    /// it in the JSON-RPC params so the subprocess doesn't have to
    /// know about providers configured elsewhere.
    pub fn with_kind(
        api_key: String,
        base_url: String,
        model: String,
        kind: tools_engine::backend::LlmProviderKind,
        sampling: LlmSampling,
    ) -> Self {
        let http = std::sync::Arc::new(
            tools_engine::http_client::build(&tools_engine::http_client::HttpClientConfig::default())
                .expect("http client"),
        );
        let key = if api_key.trim().is_empty() {
            None
        } else {
            Some(api_key)
        };
        let backend = tools_engine::backend::factory::build_backend(
            "agent".to_string(),
            kind,
            key,
            base_url,
            model,
            http,
        )
        .expect("failed to build LLM backend");
        Self {
            backend: backend.into(),
            sampling,
        }
    }

    /// Construct directly from a backend (for tests).
    pub fn from_backend(backend: Arc<dyn tools_engine::backend::LlmBackend>) -> Self {
        Self {
            backend,
            sampling: LlmSampling::default(),
        }
    }

    pub fn model_name(&self) -> &str {
        self.backend.model()
    }
    pub fn model(&self) -> &str {
        self.backend.model()
    }
    /// Borrow the sampling config — used by the agent subprocess when
    /// rebuilding the client mid-session (it must preserve the manifest-
    /// derived sampling when swapping provider/model).
    pub fn sampling(&self) -> &LlmSampling {
        &self.sampling
    }

    /// Stream a chat completion through the engine's backend.
    ///
    /// Translates the engine's `StreamEvent` broadcast into the
    /// JSON-RPC `content_delta` / `thinking_delta` notifications the
    /// agent subprocess has always emitted to its daemon parent.
    pub async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolSpec>,
        stdout: &Arc<Mutex<tokio::io::Stdout>>,
    ) -> Result<LlmResult, String> {
        
        use tokio::sync::broadcast;
        use tokio_util::sync::CancellationToken;
        use tools_engine::backend::event::StreamEvent as BEvt;

        // Stable session id for the engine's broadcast tags.
        let session_id = Uuid::new_v4();

        // Build the engine ChatMessage stream (the engine uses its own
        // ChatMessage type — convert fields by cloning; both have
        // `role` and `content` so a per-field clone is enough).
        let engine_msgs: Vec<tools_engine::backend::chat::ChatMessage> = messages
            .into_iter()
            .map(|m| tools_engine::backend::chat::ChatMessage {
                role: m.role,
                content: m.content,
                tool_calls: None,
                tool_call_id: None,
            })
            .collect();

        // Translate our ToolSpec → engine ToolSpec.
        let engine_tools: Vec<tools_engine::backend::ToolSpec> = tools
            .iter()
            .map(|t| tools_engine::backend::ToolSpec {
                kind: t.kind.clone(),
                function: tools_engine::backend::ToolFunction {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    parameters: t.function.parameters.clone(),
                },
            })
            .collect();

        // Sampling — manifest overrides defaults.
        let _max_tokens = self.sampling.max_tokens.unwrap_or(4096);
        let _temperature = self.sampling.temperature.unwrap_or(0.2);

        let (event_tx, mut event_rx) = broadcast::channel::<BEvt>(256);
        let cancel = CancellationToken::new();

        // Issue the upstream call in a background task so we can drain
        // the broadcast channel from this method (which must consume
        // the stream to drive the JSON-RPC output).
        let join = {
            // Capture the heavy params by value (clone Arc + channel) so
            // the spawned task doesn't borrow `self` (which would be a
            // borrowck error — `&self` doesn't live across `tokio::spawn`).
            let backend_arc: Arc<dyn tools_engine::backend::LlmBackend> =
                Arc::clone(&self.backend);
            let event_tx_clone = event_tx.clone();
            tokio::spawn(async move {
                backend_arc
                    .chat_stream(
                        engine_msgs,
                        engine_tools,
                        session_id,
                        &event_tx_clone,
                        &cancel,
                    )
                    .await
            })
        };

        // Drop the outer sender so the ONLY remaining `event_tx` is the
        // clone owned by the spawned backend task. When that task
        // finishes (returning `Ok(LlmResult)`), its sender drops, the
        // broadcast channel closes, and `event_rx.recv()` below returns
        // `Err(Closed)` — terminating the drain loop. Without this, the
        // loop would only exit on an explicit `BEvt::Done`, which the
        // MiniMax/OpenAI-compat backends never emit (they just return),
        // causing the agent to hang forever after streaming all content
        // and never write its final JSON-RPC response. That hang is what
        // left every chat "connecting…" / spinning with no completion.
        drop(event_tx);

        let mut full_text = String::new();
        let mut full_thinking: Option<String> = None;
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();

        while let Ok(evt) = event_rx.recv().await {
            match evt {
                BEvt::Content { text, .. } => {
                    full_text.push_str(&text);
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "content_delta",
                        "params": { "text": text },
                    });
                    Self::write_notification(stdout, &notif).await;
                }
                BEvt::Thinking { text, .. } => {
                    let text_str = text.clone();
                    full_thinking = Some(match full_thinking {
                        Some(mut t) => {
                            t.push_str(&text_str);
                            t
                        }
                        None => text_str,
                    });
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "thinking_delta",
                        "params": { "text": text },
                    });
                    Self::write_notification(stdout, &notif).await;
                }
                BEvt::ToolCall { tool, args, .. } => {
                    // Backend emits one event per call; we only get the
                    // name + args (no index). Build a single-entry
                    // vector — the LLM typically makes one tool call at
                    // a time per turn.
                    let tool_name = tool.clone();
                    let args_str = args.to_string();
                    let id = format!("call_{}", tool_calls.len());
                    tool_calls.push(ToolCallRequest {
                        id,
                        kind: "function".into(),
                        function: ToolCallFunction {
                            name: tool_name.clone(),
                            arguments: args_str.clone(),
                        },
                    });
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "tool_call",
                        "params": { "name": tool_name, "arguments": args_str },
                    });
                    Self::write_notification(stdout, &notif).await;
                }
                BEvt::Done { .. } => break,
                _ => {} // ignore other variants (Started, etc.)
            }
        }

        // Wait for the backend call to finish (might already be done).
        let result = join
            .await
            .map_err(|e| format!("backend join error: {e}"))?
            .map_err(|e| format!("backend error: {e}"))?;

        // Final text/thinking from the backend (broadcast drained only
        // deltas; the result holds the full text + tool_calls).
        full_text = result.text.clone();
        full_thinking = result.thinking.clone();
        // The engine returns tool_calls; convert each into our format.
        let our_tool_calls: Vec<ToolCallRequest> = result
            .tool_calls
            .as_ref()
            .map(|tcs| {
                tcs.iter()
                    .map(|tc| ToolCallRequest {
                        id: tc.id.clone(),
                        kind: tc.kind.clone(),
                        function: ToolCallFunction {
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(LlmResult {
            text: full_text,
            thinking: full_thinking,
            tool_calls: if our_tool_calls.is_empty() {
                None
            } else {
                Some(our_tool_calls)
            },
            tokens_out: 0, // engine emits per-call token counts separately
            streamed: true,
        })
    }

    async fn write_notification(
        stdout: &Arc<Mutex<tokio::io::Stdout>>,
        notif: &Value,
    ) {
        let mut w = stdout.lock().await;
        let _ = w.write_all(format!("{notif}\n").as_bytes()).await;
        let _ = w.flush().await;
    }
}

// ─── HTTP fallback ─────────────────────────────────────────────────────────
//
// Kept for compatibility with `chat_utility` (compaction etc.) which
// doesn't need streaming. The engine has its own `chat_utility` so
// ideally this goes away too, but leaving the path here as a safety
// net for now.
impl LlmClient {
    /// Non-streaming one-shot chat call (used by compaction). Delegates
    /// to the engine's `chat_utility`. No streaming, no retries — the
    /// caller (compaction loop) handles failure.
    pub async fn chat_utility(
        &self,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
    ) -> String {
        let engine_msgs: Vec<tools_engine::backend::chat::ChatMessage> = messages
            .into_iter()
            .map(|m| tools_engine::backend::chat::ChatMessage {
                role: m.role,
                content: m.content,
                tool_calls: None,
                tool_call_id: None,
            })
            .collect();
        let max_tokens = if max_tokens == 0 { 256 } else { max_tokens };
        self.backend.chat_utility(engine_msgs, max_tokens).await
    }
}

// ─── Internal API types (legacy) ───────────────────────────────────────────

/// Parse the `tools` array from a JSON-RPC `process` payload.
///
/// The daemon's HTTP layer hands the agent subprocess the full
/// `ToolSpec` shape — each entry includes the engine's declarative
/// metadata (`categories`, `mode_compatible`). The agent's local
/// `ToolSpec` mirror keeps those fields too (see the struct above)
/// so they survive the round-trip.
pub fn parse_tools_param(params: &Value) -> Vec<ToolSpec> {
    let tools_arr = params.get("tools").and_then(|v| v.as_array()).or_else(|| {
        params
            .get("params")
            .and_then(|p| p.get("tools"))
            .and_then(|v| v.as_array())
    });

    if let Some(arr) = tools_arr {
        arr.iter()
            .filter_map(|t| {
                let func = t.get("function");
                let name = func
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .or_else(|| t.get("name").and_then(|v| v.as_str()))?;
                let description = func
                    .and_then(|f| f.get("description"))
                    .and_then(|v| v.as_str())
                    .or_else(|| t.get("description").and_then(|v| v.as_str()))
                    .unwrap_or("");
                let parameters = func
                    .and_then(|f| f.get("parameters"))
                    .or_else(|| t.get("parameters"))
                    .cloned()
                    .unwrap_or_else(|| json!({"type":"object","properties":{}}));
                let categories = t
                    .get("categories")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let mode_compatible = t
                    .get("mode_compatible")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                Some(ToolSpec {
                    kind: "function".into(),
                    function: ToolFunction {
                        name: name.to_string(),
                        description: description.to_string(),
                        parameters,
                    },
                    categories,
                    mode_compatible,
                })
            })
            .collect()
    } else {
        Vec::new()
    }
}
