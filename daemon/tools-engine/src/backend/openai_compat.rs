//! OpenAI-compatible backend — `LlmBackend` implementation for any provider
//! speaking the OpenAI `POST /chat/completions` + SSE protocol: OpenAI,
//! OpenRouter, Mistral, Ollama.
//!
//! Differences from the MiniMax backend (EP-0009-02):
//! - `api_key` is optional (Ollama needs no auth; header omitted when `None`)
//! - `provider_id()` returns the configured provider id (e.g. `"openrouter"`)
//! - `reasoning_content` deltas (OpenAI o-series) are emitted as `Event::Thinking`
//! - `tool_choice` can be disabled for providers that reject it (`send_tool_choice`)
//! - `chat_utility` uses `self.model` (not a hardcoded model)

use super::{strip_thinking, LlmBackend, LlmResult, ToolCallFunction, ToolCallRequest, ToolSpec};
use crate::backend::{ChatMessage, StreamEvent};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// OpenAI-compatible LLM backend.
pub struct OpenAiCompatBackend {
    pub http: std::sync::Arc<reqwest::Client>,
    pub provider_id_value: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub extra_headers: HashMap<String, String>,
    pub send_tool_choice: bool,
}

impl OpenAiCompatBackend {
    pub fn new(
        http: std::sync::Arc<reqwest::Client>,
        provider_id: String,
        api_key: Option<String>,
        base_url: String,
        model: String,
        extra_headers: HashMap<String, String>,
        send_tool_choice: bool,
    ) -> Self {
        Self {
            http,
            provider_id_value: provider_id,
            api_key,
            base_url,
            model,
            extra_headers,
            send_tool_choice,
        }
    }
}

#[async_trait::async_trait]
impl LlmBackend for OpenAiCompatBackend {
    fn provider_id(&self) -> &str {
        &self.provider_id_value
    }

    fn model(&self) -> &str {
        &self.model
    }

    /// Streaming chat call — emits Event::Content / Event::Thinking per chunk
    /// as they arrive from the API (real token-by-token streaming).
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolSpec>,
        session_id: Uuid,
        event_tx: &broadcast::Sender<crate::backend::StreamEvent>,
        cancel: &CancellationToken,
    ) -> Result<LlmResult, String> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 4096,
            temperature: 0.7,
            stream: true,
            tools: tools.clone(),
            tool_choice: if tools.is_empty() || !self.send_tool_choice {
                None
            } else {
                Some("auto".into())
            },
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut request = self
            .http
            .post(&url)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {key}"));
        }
        for (k, v) in &self.extra_headers {
            request = request.header(k, v);
        }
        let resp = request
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
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut full_text = String::new();
        let mut full_thinking = String::new();
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();

        while let Some(chunk_result) = stream.next().await {
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
                    break;
                }

                let Ok(chunk) = serde_json::from_str::<StreamChunk>(rest) else {
                    continue;
                };

                let Some(choice) = chunk.choices.first() else {
                    continue;
                };

                // reasoning_content (OpenAI o-series) → Thinking events
                if let Some(thinking) = &choice.delta.reasoning_content {
                    full_thinking.push_str(thinking);
                    let _ = event_tx.send(StreamEvent::Thinking {
                        session_id,
                        text: thinking.clone(),
                    });
                }

                // content → Content events
                if let Some(content) = &choice.delta.content {
                    full_text.push_str(content);
                    let _ = event_tx.send(StreamEvent::Content {
                        session_id,
                        text: content.clone(),
                    });
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

        // Process any remaining data in the buffer
        if !buf.trim().is_empty() {
            for raw_line in buf.lines() {
                let line = raw_line.trim();
                if let Some(rest) = line.strip_prefix("data: ") {
                    if rest != "[DONE]" && !rest.is_empty() {
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(rest) {
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(thinking) = &choice.delta.reasoning_content {
                                    full_thinking.push_str(thinking);
                                    let _ = event_tx.send(StreamEvent::Thinking {
                                        session_id,
                                        text: thinking.clone(),
                                    });
                                }
                                if let Some(content) = &choice.delta.content {
                                    full_text.push_str(content);
                                    let _ = event_tx.send(StreamEvent::Content {
                                        session_id,
                                        text: content.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Strip <think>...</think> from content (some providers emit tags in content)
        let (thinking, clean_text) = strip_thinking(&full_text);
        // Prefer explicit reasoning_content if present; otherwise tags from content.
        let thinking = if full_thinking.is_empty() {
            thinking
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
            model: self.model.clone(),
            messages,
            max_tokens,
            temperature: 0.3,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut request = self
            .http
            .post(&url)
            .header("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {key}"));
        }
        for (k, v) in &self.extra_headers {
            request = request.header(k, v);
        }

        let resp = match request.json(&req).send().await {
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
    /// OpenAI o-series reasoning content (emitted as Event::Thinking).
    #[serde(default)]
    reasoning_content: Option<String>,
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn backend_for(server: &MockServer, api_key: Option<String>) -> OpenAiCompatBackend {
        OpenAiCompatBackend::new(
            std::sync::Arc::new(reqwest::Client::new()),
            "openrouter".to_string(),
            api_key,
            server.uri(),
            "test-model".to_string(),
            HashMap::new(),
            true,
        )
    }

    fn events_channel() -> broadcast::Sender<crate::backend::StreamEvent> {
        broadcast::channel(64).0
    }

    #[tokio::test]
    async fn openai_sends_auth_and_parses_content_and_reasoning() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"hola \"}}]}\n\
                 data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"mundo\",\"reasoning_content\":\"pensando...\"}}]}\n\
                 data: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let backend = backend_for(&server, Some("test-key".to_string()));
        let tx = events_channel();
        let mut rx = tx.subscribe();
        let cancel = CancellationToken::new();

        let result = backend
            .chat_stream(vec![], vec![], uuid::Uuid::new_v4(), &tx, &cancel)
            .await
            .expect("stream should succeed");

        assert_eq!(result.text, "hola mundo");
        assert_eq!(result.thinking.as_deref(), Some("pensando..."));

        // Events: Content(hmm) -> Thinking -> Content — order depends on arrival.
        let mut got_content = String::new();
        let mut got_thinking = String::new();
        while let Ok(ev) = rx.try_recv() {
            match ev {
                crate::backend::StreamEvent::Content { text, .. } => got_content.push_str(&text),
                crate::backend::StreamEvent::Thinking { text, .. } => got_thinking.push_str(&text),
                _ => {}
            }
        }
        assert_eq!(got_content, "hola mundo");
        assert_eq!(got_thinking, "pensando...");
    }

    #[tokio::test]
    async fn ollama_sends_no_auth_header() {
        let server = MockServer::start().await;

        // Mount a responder and verify afterwards that no Authorization header was sent.
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"}}]}\n\
                 data: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let backend = backend_for(&server, None);
        let tx = events_channel();
        let cancel = CancellationToken::new();

        let result = backend
            .chat_stream(vec![], vec![], uuid::Uuid::new_v4(), &tx, &cancel)
            .await
            .expect("ollama stream should succeed");

        assert_eq!(result.text, "ok");

        let received = server.received_requests().await.unwrap();
        let headers = &received[0].headers;
        assert!(
            !headers.contains_key("authorization"),
            "ollama request must not carry an Authorization header"
        );
    }

    #[tokio::test]
    async fn tool_calls_accumulate_by_index() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"id\":\"1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"city\\\":\\\"santiago\\\"}\"}}]}}]}\n\
                 data: [DONE]\n",
            ))
            .mount(&server)
            .await;

        let backend = backend_for(&server, Some("test-key".to_string()));
        let tx = events_channel();
        let cancel = CancellationToken::new();

        let result = backend
            .chat_stream(vec![], vec![], uuid::Uuid::new_v4(), &tx, &cancel)
            .await
            .expect("stream should succeed");

        let calls = result.tool_calls.expect("tool call expected");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].function.name, "get_weather");
        assert_eq!(calls[0].function.arguments, r#"{"city":"santiago"}"#);
    }
}
