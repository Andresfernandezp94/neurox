//! MiniMax backend — `LlmBackend` implementation for the MiniMax API.
//!
//! Streaming SSE via `POST /chat/completions` (OpenAI-compatible shape).
//! Uses `bytes_stream()` to process SSE events incrementally as they arrive
//! from the API (real token-by-token streaming). `<think>` tags are split in
//! real-time into Thinking/Content events. Tool calls accumulate per index.
//! `chat_utility` uses the hardcoded `MiniMax-M2.7-highspeed` model.

use super::{strip_thinking, LlmBackend, LlmResult, ToolCallFunction, ToolCallRequest, ToolSpec};
use crate::backend::{ChatMessage, StreamEvent};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Strip `<think>...</think>` content inline from a streaming chunk.
/// `in_think` is the mutable state tracking whether we're currently inside
/// a think block — carried across chunks within the same request.
/// Returns only the content OUTSIDE think blocks.
fn strip_think_inline(chunk: &str, in_think: &mut bool) -> String {
    let mut result = String::new();
    let mut rest = chunk;
    loop {
        if *in_think {
            if let Some(end) = rest.find("</think>") {
                // Skip everything up to and including </think>
                rest = &rest[(end + "</think>".len())..];
                *in_think = false;
            } else {
                // Still inside think block — discard entire chunk
                break;
            }
        } else if let Some(start) = rest.find("<think>") {
            // Emit content before the tag
            result.push_str(&rest[..start]);
            rest = &rest[(start + "<think>".len())..];
            *in_think = true;
        } else {
            // No tags — all content
            result.push_str(rest);
            break;
        }
    }
    result
}

/// MiniMax LLM backend.
pub struct MiniMaxBackend {
    pub http: std::sync::Arc<reqwest::Client>,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl MiniMaxBackend {
    pub fn new(
        http: std::sync::Arc<reqwest::Client>,
        api_key: String,
        base_url: String,
        model: String,
    ) -> Self {
        Self {
            http,
            api_key,
            base_url,
            model,
        }
    }
}

#[async_trait::async_trait]
impl LlmBackend for MiniMaxBackend {
    fn provider_id(&self) -> &str {
        "minimax"
    }

    fn model(&self) -> &str {
        &self.model
    }

    /// Streaming chat call — emits Event::Thinking / Event::Content per chunk
    /// to the broadcast channel as they arrive from the API (real streaming).
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolSpec>,
        session_id: Uuid,
        event_tx: &broadcast::Sender<crate::backend::StreamEvent>,
        cancel: &CancellationToken,
    ) -> Result<LlmResult, String> {
        let messages_len = messages.len();
        let req = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 4096,
            temperature: 0.7,
            stream: true,
            tools: tools.clone(),
            tool_choice: if tools.is_empty() {
                None
            } else {
                Some("auto".into())
            },
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        tracing::debug!(
            model = %self.model,
            url = %url,
            messages = messages_len,
            "minimax chat_stream: sending request"
        );
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("api call: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let msg = match serde_json::from_str::<ApiError>(&body) {
                Ok(e) => format!("api returned {}: {}", status, e.error.message),
                Err(_) => format!("api returned {status}: {body}"),
            };
            return Err(msg);
        }

        // ─── Stream processing: read bytes incrementally ────────────────
        // Instead of `resp.text().await` (which buffers the entire response),
        // we use `bytes_stream()` to process SSE events as they arrive from
        // the API — enabling real token-by-token streaming to the frontend.

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut full_text = String::new();
        let mut full_thinking = String::new();
        let mut in_think_block = false;
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            // Check cancellation between chunks
            if cancel.is_cancelled() {
                return Err("cancelled".to_string());
            }

            let bytes = chunk_result.map_err(|e| format!("stream read: {e}"))?;
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // Process complete lines from the buffer
            while let Some(newline_pos) = buf.find('\n') {
                let raw_line = buf[..newline_pos].to_string();
                buf = buf[newline_pos + 1..].to_string();
                let line = raw_line.trim();

                if line.is_empty() {
                    continue;
                }

                let Some(rest) = line.strip_prefix("data: ") else {
                    continue;
                };

                if rest == "[DONE]" {
                    // Stream complete
                    break;
                }

                let Ok(chunk) = serde_json::from_str::<StreamChunk>(rest) else {
                    continue;
                };

                let Some(choice) = chunk.choices.first() else {
                    continue;
                };

                // Process reasoning (MiniMax provides this separately)
                if let Some(reasoning) = &choice.delta.reasoning {
                    if !reasoning.is_empty() {
                        full_thinking.push_str(reasoning);
                        let _ = event_tx.send(StreamEvent::Thinking {
                            session_id,
                            text: reasoning.clone(),
                        });
                    }
                }

                // Process content — strip any <think>...</think> tags
                // that MiniMax may include inline in the content field.
                if let Some(content) = &choice.delta.content {
                    full_text.push_str(content);
                    // Extract only the non-thinking portions for Content events
                    let clean = strip_think_inline(content, &mut in_think_block);
                    if !clean.is_empty() {
                        let _ = event_tx.send(StreamEvent::Content {
                            session_id,
                            text: clean,
                        });
                    }
                }

                // Accumulate tool calls
                if let Some(stream_calls) = &choice.delta.tool_calls {
                    for stc in stream_calls {
                        while tool_calls.len() <= stc.index as usize {
                            tool_calls.push(ToolCallRequest {
                                id: String::new(),
                                kind: "function".into(),
                                function: ToolCallFunction {
                                    name: String::new(),
                                    arguments: String::new(),
                                },
                            });
                        }
                        let entry = &mut tool_calls[stc.index as usize];
                        if let Some(id) = &stc.id {
                            entry.id.clone_from(id);
                        }
                        if let Some(k) = &stc.kind {
                            entry.kind.clone_from(k);
                        }
                        if let Some(f) = &stc.function {
                            if let Some(n) = &f.name {
                                entry.function.name.push_str(n);
                            }
                            entry.function.arguments.push_str(&f.arguments);
                        }
                    }
                }
            }
        }

        // Process any remaining data in the buffer (no trailing newline)
        if !buf.trim().is_empty() {
            for raw_line in buf.lines() {
                let line = raw_line.trim();
                if let Some(rest) = line.strip_prefix("data: ") {
                    if rest != "[DONE]" && !rest.is_empty() {
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(rest) {
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(reasoning) = &choice.delta.reasoning {
                                    if !reasoning.is_empty() {
                                        full_thinking.push_str(reasoning);
                                        let _ = event_tx.send(StreamEvent::Thinking {
                                            session_id,
                                            text: reasoning.clone(),
                                        });
                                    }
                                }
                                if let Some(content) = &choice.delta.content {
                                    full_text.push_str(content);
                                    let clean = strip_think_inline(content, &mut in_think_block);
                                    if !clean.is_empty() {
                                        let _ = event_tx.send(StreamEvent::Content {
                                            session_id,
                                            text: clean,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Strip <think>...</think> from the accumulated full text for the final result
        let (_thinking_tags, clean_text) = strip_thinking(&full_text);
        // Prefer explicit reasoning if present; otherwise use tags from content.
        let thinking = if full_thinking.is_empty() {
            _thinking_tags
        } else {
            Some(full_thinking)
        };

        Ok(LlmResult {
            text: clean_text,
            thinking,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            tokens_out: full_text.len(),
        })
    }

    /// Non-streaming utility call (for compaction summaries).
    async fn chat_utility(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> String {
        let req = ChatRequest {
            model: "MiniMax-M2.7-highspeed".to_string(),
            messages,
            max_tokens,
            temperature: 0.3,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = match self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("utility call failed: {e}");
                return String::new();
            }
        };

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!(len = body.len(), "utility call error");
            return String::new();
        }

        match resp.json::<NonStreamResponse>().await {
            Ok(parsed) => parsed
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default(),
            Err(e) => {
                tracing::warn!("utility parse error: {e}");
                String::new()
            }
        }
    }
}

// ─── Internal API types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    model: String,
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[allow(dead_code)]
    index: u32,
    delta: StreamDelta,
    #[allow(dead_code)]
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamDelta {
    #[allow(dead_code)]
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    /// MiniMax M3 emits reasoning separately from content — use this
    /// directly as Thinking events instead of parsing `<think>` tags.
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCall {
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    function: Option<StreamToolCallFunction>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamToolCallFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct NonStreamResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    model: String,
    choices: Vec<NonStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct NonStreamChoice {
    message: NonStreamMessage,
}

#[derive(Debug, Deserialize)]
struct NonStreamMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: String,
}
