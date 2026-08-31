//! Anthropic backend — `LlmBackend` implementation for Claude.
//!
//! Anthropic uses a different protocol than OpenAI: `POST /v1/messages`,
//! auth via `x-api-key` header, versioned via `anthropic-version`, the system
//! prompt goes OUTSIDE the messages array, tools use `input_schema` (not
//! `parameters`), and streaming is event-based (`content_block_start`,
//! `content_block_delta`, `content_block_stop`, `message_delta`,
//! `message_stop`). Tool calls arrive as a `content_block_start` with type
//! `tool_use` and their input is accumulated via `input_json_delta`. Claude's
//! native extended thinking is translated to `Event::Thinking`.

use super::{strip_thinking, LlmBackend, LlmResult, ToolCallFunction, ToolCallRequest, ToolSpec};
use crate::backend::ChatMessage;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Anthropic (Claude) LLM backend.
pub struct AnthropicBackend {
    pub http: std::sync::Arc<reqwest::Client>,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub anthropic_version: String,
}

impl AnthropicBackend {
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
            anthropic_version: "2023-06-01".into(),
        }
    }
}

#[async_trait::async_trait]
impl LlmBackend for AnthropicBackend {
    fn provider_id(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    /// Streaming chat call — emits Event::Content / Event::Thinking per delta
    /// as they arrive from the API (real token-by-token streaming).
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolSpec>,
        session_id: Uuid,
        event_tx: &broadcast::Sender<crate::backend::StreamEvent>,
        cancel: &CancellationToken,
    ) -> Result<LlmResult, String> {
        // Anthropic requires system OUTSIDE messages, and only user/assistant
        // roles inside the array.
        let (system, api_messages) = split_system_messages(messages);
        let api_tools: Vec<AnthropicTool> = tools.iter().map(AnthropicTool::from_spec).collect();

        let req = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            temperature: 0.7,
            stream: true,
            system: if system.is_empty() {
                None
            } else {
                Some(system)
            },
            messages: api_messages,
            tools: api_tools,
        };

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.anthropic_version)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("api call: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let msg = match serde_json::from_str::<ApiErrorResponse>(&body) {
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
        let mut in_progress: Vec<Option<ToolCallInProgress>> = Vec::new();
        let mut pending_event: Option<String> = None;

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

                if let Some(name) = line.strip_prefix("event: ") {
                    pending_event = Some(name.to_string());
                    continue;
                }

                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };

                let event_name = pending_event.take().unwrap_or_default();
                let Ok(ev) = serde_json::from_str::<AnthropicSseEvent>(data) else {
                    continue;
                };

                match ev {
                    AnthropicSseEvent::ContentBlockStart {
                        index,
                        content_block,
                    } => match content_block {
                        ContentBlockStartBlock::Text { text } => {
                            full_text.push_str(&text);
                            let _ = event_tx.send(crate::backend::StreamEvent::Content { session_id, text });
                        }
                        ContentBlockStartBlock::ToolUse { id, name, input } => {
                            while in_progress.len() <= index {
                                in_progress.push(None);
                            }
                            in_progress[index] = Some(ToolCallInProgress {
                                id,
                                name,
                                partial_json: String::new(),
                                start_input: input,
                                finalized: false,
                            });
                        }
                        ContentBlockStartBlock::Thinking { thinking } => {
                            full_thinking.push_str(&thinking);
                            let _ = event_tx.send(crate::backend::StreamEvent::Thinking {
                                session_id,
                                text: thinking,
                            });
                        }
                    },
                    AnthropicSseEvent::ContentBlockDelta { index, delta } => match delta {
                        ContentBlockDelta::TextDelta { text } => {
                            full_text.push_str(&text);
                            let _ = event_tx.send(crate::backend::StreamEvent::Content { session_id, text });
                        }
                        ContentBlockDelta::InputJsonDelta { partial_json } => {
                            if let Some(Some(call)) = in_progress.get_mut(index) {
                                call.partial_json.push_str(&partial_json);
                            }
                        }
                        ContentBlockDelta::ThinkingDelta { thinking } => {
                            full_thinking.push_str(&thinking);
                            let _ = event_tx.send(crate::backend::StreamEvent::Thinking {
                                session_id,
                                text: thinking,
                            });
                        }
                    },
                    AnthropicSseEvent::ContentBlockStop { index } => {
                        if let Some(Some(call)) = in_progress.get_mut(index) {
                            call.finalized = true;
                        }
                    }
                    AnthropicSseEvent::MessageDelta { .. } => {}
                    AnthropicSseEvent::MessageStop => {}
                    AnthropicSseEvent::Ping => {}
                    AnthropicSseEvent::Error { error } => {
                        return Err(format!("stream error: {}", error.message));
                    }
                }
                let _ = event_name;
            }
        }

        // Process remaining buffer
        if !buf.trim().is_empty() {
            for raw_line in buf.lines() {
                let line = raw_line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some(name) = line.strip_prefix("event: ") {
                    pending_event = Some(name.to_string());
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    let _event_name = pending_event.take().unwrap_or_default();
                    if let Ok(AnthropicSseEvent::ContentBlockDelta { index: _, delta }) =
                        serde_json::from_str::<AnthropicSseEvent>(data)
                    {
                        match delta {
                            ContentBlockDelta::TextDelta { text } => {
                                full_text.push_str(&text);
                                let _ = event_tx.send(crate::backend::StreamEvent::Content { session_id, text });
                            }
                            ContentBlockDelta::ThinkingDelta { thinking } => {
                                full_thinking.push_str(&thinking);
                                let _ = event_tx.send(crate::backend::StreamEvent::Thinking {
                                    session_id,
                                    text: thinking,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Build tool calls from finalized in-progress entries
        let tool_calls: Vec<ToolCallRequest> = in_progress
            .into_iter()
            .flatten()
            .map(|call| {
                let arguments = call.final_arguments();
                ToolCallRequest {
                    id: call.id,
                    kind: "function".into(),
                    function: ToolCallFunction {
                        name: call.name,
                        arguments,
                    },
                }
            })
            .collect();

        // Strip <think>...</think> from content as a fallback
        let (thinking_tags, clean_text) = strip_thinking(&full_text);
        let thinking = if full_thinking.is_empty() {
            thinking_tags
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
        let (system, api_messages) = split_system_messages(messages);
        let req = MessagesRequest {
            model: self.model.clone(),
            max_tokens,
            temperature: 0.3,
            stream: false,
            system: if system.is_empty() {
                None
            } else {
                Some(system)
            },
            messages: api_messages,
            tools: Vec::new(),
        };

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = match self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.anthropic_version)
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

        match resp.json::<MessagesResponse>().await {
            Ok(parsed) => parsed
                .content
                .iter()
                .filter_map(|b| b.text.clone())
                .collect::<Vec<_>>()
                .concat(),
            Err(e) => {
                tracing::warn!("utility parse error: {e}");
                String::new()
            }
        }
    }
}

/// Accumulated tool call while streaming.
struct ToolCallInProgress {
    id: String,
    name: String,
    partial_json: String,
    start_input: Value,
    finalized: bool,
}

impl ToolCallInProgress {
    /// Produce the JSON-string arguments: prefer the accumulated
    /// `input_json_delta` (re-serialized compact if parseable, raw otherwise);
    /// fall back to the `input` object from `content_block_start`.
    fn final_arguments(&self) -> String {
        if !self.partial_json.is_empty() {
            match serde_json::from_str::<Value>(&self.partial_json) {
                Ok(v) => v.to_string(),
                Err(_) => self.partial_json.clone(),
            }
        } else if self.start_input.is_object() && !self.start_input.as_object().unwrap().is_empty()
        {
            self.start_input.to_string()
        } else {
            String::new()
        }
    }
}

/// Split ChatMessages into (system_text, api_messages) — Anthropic requires
/// system outside the array and only user/assistant roles inside it.
fn split_system_messages(messages: Vec<ChatMessage>) -> (String, Vec<AnthropicMessage>) {
    let mut system = String::new();
    let mut api_messages = Vec::new();
    for m in messages {
        if m.role == "system" {
            if let Some(text) = m.content {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&text);
            }
        } else {
            api_messages.push(AnthropicMessage {
                role: m.role,
                content: m.content.unwrap_or_default(),
            });
        }
    }
    (system, api_messages)
}

// ─── Internal API types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: Value,
}

impl AnthropicTool {
    fn from_spec(spec: &ToolSpec) -> Self {
        Self {
            name: spec.function.name.clone(),
            description: spec.function.description.clone(),
            input_schema: spec.function.parameters.clone(),
        }
    }
}

/// SSE stream events — parsed from the `data:` payloads, type-driven.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicSseEvent {
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStartBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        index: usize,
    },
    #[serde(rename = "message_delta")]
    MessageDelta {
        #[allow(dead_code)]
        delta: MessageDeltaBody,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    Ping,
    Error {
        error: ApiErrorBody,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlockStartBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
enum ContentBlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
}

#[derive(Debug, Deserialize)]
struct MessageDeltaBody {
    #[allow(dead_code)]
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: String,
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::backend::StreamEvent;
    use super::*;
    use serde_json::json;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn backend_for(server: &MockServer, api_key: &str) -> AnthropicBackend {
        AnthropicBackend::new(
            std::sync::Arc::new(reqwest::Client::new()),
            api_key.into(),
            server.uri(),
            "claude-test".into(),
        )
    }

    fn events_channel() -> broadcast::Sender<crate::backend::StreamEvent> {
        broadcast::channel(64).0
    }

    fn tool_spec() -> ToolSpec {
        ToolSpec {
            kind: "function".into(),
            function: super::super::ToolFunction {
                name: "get_weather".into(),
                description: "Obtiene el clima".into(),
                parameters: json!({
                    "type": "object",
                    "properties": { "city": { "type": "string" } },
                    "required": ["city"]
                }),
            },
        }
    }

    fn messages_with_system() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: "system".into(),
                content: Some("Eres un asistente útil".into()),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".into(),
                content: Some("¿Qué día es?".into()),
                tool_calls: None,
                tool_call_id: None,
            },
        ]
    }

    #[tokio::test]
    async fn request_body_splits_system_and_uses_input_schema() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "event: message_start\n\
                 data: {\"type\":\"message_start\",\"message\":{\"id\":\"m_1\"}}\n\
                 event: content_block_start\n\
                 data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hola\"}}\n\
                 event: content_block_stop\n\
                 data: {\"type\":\"content_block_stop\",\"index\":0}\n\
                 event: message_delta\n\
                 data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null}}\n\
                 event: message_stop\n\
                 data: {\"type\":\"message_stop\"}\n",
            ))
            .mount(&server)
            .await;

        let backend = backend_for(&server, "test-key");
        let tx = events_channel();
        let cancel = CancellationToken::new();
        let result = backend
            .chat_stream(
                messages_with_system(),
                vec![tool_spec()],
                uuid::Uuid::new_v4(),
                &tx,
                &cancel,
            )
            .await
            .expect("stream should succeed");

        assert_eq!(result.text, "hola");

        // Inspect the request body — system must be outside messages, tools use input_schema.
        let received = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&received[0].body).expect("valid json body");
        assert_eq!(body["system"], "Eres un asistente útil");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert!(msgs[0].get("content").is_some());
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools[0]["name"], "get_weather");
        assert_eq!(tools[0]["input_schema"]["type"], "object");
        assert_eq!(tools[0]["input_schema"]["required"][0], "city");
        assert!(
            body.get("parameters").is_none(),
            "anthropic uses input_schema"
        );
    }

    #[tokio::test]
    async fn stream_accumulates_content_and_tool_input_json_delta() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "event: content_block_start\n\
                 data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Voy a consultar el clima de \"}}\n\
                 event: content_block_stop\n\
                 data: {\"type\":\"content_block_stop\",\"index\":0}\n\
                 event: content_block_start\n\
                 data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"get_weather\",\"input\":{}}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\": \\\"san\"}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"tiago\\\"}\"}}\n\
                 event: content_block_stop\n\
                 data: {\"type\":\"content_block_stop\",\"index\":1}\n\
                 event: message_delta\n\
                 data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null}}\n\
                 event: message_stop\n\
                 data: {\"type\":\"message_stop\"}\n",
            ))
            .mount(&server)
            .await;

        let backend = backend_for(&server, "test-key");
        let tx = events_channel();
        let mut rx = tx.subscribe();
        let cancel = CancellationToken::new();
        let result = backend
            .chat_stream(
                vec![],
                vec![tool_spec()],
                uuid::Uuid::new_v4(),
                &tx,
                &cancel,
            )
            .await
            .expect("stream should succeed");

        assert_eq!(result.text, "Voy a consultar el clima de ");

        let calls = result.tool_calls.expect("tool call expected");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_01");
        assert_eq!(calls[0].function.name, "get_weather");
        assert_eq!(calls[0].function.arguments, r#"{"city":"santiago"}"#);

        let mut got_content = String::new();
        while let Ok(ev) = rx.try_recv() {
            if let StreamEvent::Content { text, .. } = ev {
                got_content.push_str(&text);
            }
        }
        assert_eq!(got_content, "Voy a consultar el clima de ");
    }

    #[tokio::test]
    async fn thinking_deltas_emit_thinking_not_content() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "event: content_block_start\n\
                 data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"pensando en \"}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"la respuesta\"}}\n\
                 event: content_block_stop\n\
                 data: {\"type\":\"content_block_stop\",\"index\":0}\n\
                 event: content_block_start\n\
                 data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
                 event: content_block_delta\n\
                 data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"respuesta final\"}}\n\
                 event: content_block_stop\n\
                 data: {\"type\":\"content_block_stop\",\"index\":1}\n\
                 event: message_stop\n\
                 data: {\"type\":\"message_stop\"}\n",
            ))
            .mount(&server)
            .await;

        let backend = backend_for(&server, "test-key");
        let tx = events_channel();
        let mut rx = tx.subscribe();
        let cancel = CancellationToken::new();
        let result = backend
            .chat_stream(vec![], vec![], uuid::Uuid::new_v4(), &tx, &cancel)
            .await
            .expect("stream should succeed");

        assert_eq!(result.text, "respuesta final");
        assert_eq!(result.thinking.as_deref(), Some("pensando en la respuesta"));

        let mut got_thinking = String::new();
        let mut got_content = String::new();
        while let Ok(ev) = rx.try_recv() {
            match ev {
                crate::backend::StreamEvent::Thinking { text, .. } => got_thinking.push_str(&text),
                crate::backend::StreamEvent::Content { text, .. } => got_content.push_str(&text),
                _ => {}
            }
        }
        assert_eq!(got_thinking, "pensando en la respuesta");
        assert_eq!(got_content, "respuesta final");
    }
}
