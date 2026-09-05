use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::events::Event;
use crate::protocols::{AgentHealth, AgentProtocol, AgentRequest, AgentResponse, HealthStatus};

pub struct JsonRpcStdio {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<ChildStdout>>>,
    next_id: Mutex<u64>,
}

impl JsonRpcStdio {
    pub fn new(mut child: Child) -> Self {
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().map(BufReader::new);
        Self {
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(stdout),
            next_id: Mutex::new(1),
        }
    }

    async fn next_id(&self) -> u64 {
        let mut g = self.next_id.lock().await;
        let id = *g;
        *g += 1;
        id
    }

    async fn send_request(
        &self,
        method: &str,
        params: Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<Value> {
        if cancel.is_cancelled() {
            anyhow::bail!("request cancelled before send");
        }
        let id = self.next_id().await;
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = format!("{}\n", serde_json::to_string(&req)?);

        {
            let mut stdin_guard = self.stdin.lock().await;
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("stdin already closed"))?;
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await?;
        }

        let mut response_line = String::new();
        {
            let mut stdout_guard = self.stdout.lock().await;
            let stdout = stdout_guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("stdout already closed"))?;
            tokio::select! {
                biased;
                () = cancel.cancelled() => anyhow::bail!("cancelled while waiting for response"),
                res = stdout.read_line(&mut response_line) => {
                    res?;
                }
            }
        }

        let trimmed = response_line.trim();
        if trimmed.is_empty() {
            anyhow::bail!("empty response from agent");
        }
        let resp: Value = serde_json::from_str(trimmed)?;
        if let Some(err) = resp.get("error") {
            anyhow::bail!("agent error: {err}");
        }
        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    }
}

#[async_trait]
impl AgentProtocol for JsonRpcStdio {
    async fn call(
        &self,
        req: AgentRequest,
        cancel: CancellationToken,
    ) -> anyhow::Result<AgentResponse> {
        let params = serde_json::json!({
            "session_id": req.session_id.to_string(),
            "params": req.params,
        });
        match self.send_request(&req.method, params, cancel).await {
            Ok(result) => Ok(AgentResponse {
                session_id: req.session_id,
                ok: true,
                result: Some(result),
                error: None,
            }),
            Err(e) => Ok(AgentResponse {
                session_id: req.session_id,
                ok: false,
                result: None,
                error: Some(e.to_string()),
            }),
        }
    }

    async fn stream(
        &self,
        req: AgentRequest,
        cancel: CancellationToken,
        tx: tokio::sync::mpsc::Sender<Event>,
    ) -> anyhow::Result<AgentResponse> {
        if cancel.is_cancelled() {
            anyhow::bail!("stream cancelled before send");
        }
        let id = self.next_id().await;
        let rpc_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": &req.method,
            "params": serde_json::json!({
                "session_id": req.session_id.to_string(),
                "params": req.params,
            }),
        });
        let line = format!("{}\n", serde_json::to_string(&rpc_req)?);

        {
            let mut stdin_guard = self.stdin.lock().await;
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("stdin already closed"))?;
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await?;
        }

        // Read lines until we get a response (has "id" field).
        // Lines without "id" are notifications forwarded as Content events.
        loop {
            if cancel.is_cancelled() {
                anyhow::bail!("stream cancelled while waiting for response");
            }
            let mut response_line = String::new();
            {
                let mut stdout_guard = self.stdout.lock().await;
                let stdout = stdout_guard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("stdout already closed"))?;
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => anyhow::bail!("stream cancelled while reading"),
                    res = stdout.read_line(&mut response_line) => {
                        res?;
                    }
                }
            }

            let trimmed = response_line.trim();
            // EP-2026-09-05: empty lines happen intermittently — the
            // agent's stdout isn't perfectly clean between writes
            // (the LLM error path occasionally emits a stray newline).
            // Skip them and keep reading instead of bailing; if the
            // stream is truly dead, the cancellation token or
            // broken pipe below will catch it.
            if trimmed.is_empty() {
                tracing::debug!("empty line from agent during stream; skipping");
                continue;
            }

            let msg: Value = serde_json::from_str(trimmed)?;

            // If the message has an "id", it's the final response
            if msg.get("id").is_some() && !msg.get("id").unwrap().is_null() {
                if let Some(err) = msg.get("error") {
                    anyhow::bail!("agent error: {err}");
                }
                let result = msg.get("result").cloned().unwrap_or(Value::Null);
                return Ok(AgentResponse {
                    session_id: req.session_id,
                    ok: true,
                    result: Some(result),
                    error: None,
                });
            }

            // Otherwise it's a notification — forward as Event::Content
            if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
                // Special case: `compaction_failed` carries a `reason`
                // string (not `text`). Read it before the generic
                // text path so the message field is well-defined.
                if method == "compaction_failed" {
                    if let Some(reason) = msg
                        .get("params")
                        .and_then(|p| p.get("reason"))
                        .and_then(|v| v.as_str())
                    {
                        let _ = tx
                            .send(Event::CompactionFailed {
                                session_id: req.session_id,
                                reason: reason.to_string(),
                            })
                            .await;
                    }
                    continue;
                }
                if let Some(text) = msg
                    .get("params")
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                {
                    let event = match method {
                        // EP-2026-09-05 (stream seq): `seq` is a
                        // placeholder here (0). The real per-session
                        // sequence is stamped in the router's forwarder
                        // task right before the event is broadcast on
                        // the bus, so all four stream event types share
                        // one strictly-increasing sequence per session.
                        "content_delta" => Event::Content {
                            session_id: req.session_id,
                            text: text.to_string(),
                            seq: 0,
                        },
                        "thinking_delta" => Event::Thinking {
                            session_id: req.session_id,
                            text: text.to_string(),
                            seq: 0,
                        },
                        _ => continue, // unknown notification type — ignore
                    };
                    let _ = tx.send(event).await;
                }
            }
        }
    }

    async fn health(&self) -> AgentHealth {
        let child_alive = self.child.lock().await.is_some();
        if child_alive {
            AgentHealth {
                status: HealthStatus::Healthy,
                detail: Some("subprocess running".to_string()),
            }
        } else {
            AgentHealth {
                status: HealthStatus::Unhealthy,
                detail: Some("subprocess not running".to_string()),
            }
        }
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        let mut child_guard = self.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            let _ = child.start_kill();
        }
        *child_guard = None;
        Ok(())
    }

    /// EP-0013 T-004: take the Child out so the supervisor can wait()
    /// on it and detect exit. The Child is moved out; the protocol is
    /// left without a child (subsequent calls will fail until shutdown).
    async fn take_child(&self) -> Option<tokio::process::Child> {
        let mut g = self.child.lock().await;
        g.take()
    }
}
