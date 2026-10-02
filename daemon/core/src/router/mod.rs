use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;


use crate::events::Event;
// use crate::router::http::provider_api_key; // EP-2026-08-15: removed — chat now goes via subprocess, not llmd.
use crate::protocols::{AgentRequest, AgentResponse};
use tools_engine::tools::ToolCall;

pub mod handlers;
pub mod http;
pub mod middleware;
pub mod routes_composer;
pub mod state;
pub mod ws;

pub use routes_composer::router;
pub use state::AppState;

/// Maximum number of tool_call iterations per single user message.
/// Prevents infinite loops where the LLM keeps calling tools without
/// converging (e.g. read_file → list_dir → read_file → ...).
/// Soft cap on tool-call iterations per user message. EP-2026-08-19
/// (v2 manifest): the agent's `stop_conditions.max_iterations` is
/// surfaced via `NEUROX_MAX_TOOL_ITERATIONS` env var (set by the agent
/// subprocess at startup). Falls back to 10 for agents without the
/// manifest field.
pub fn max_tool_iterations() -> u32 {
    std::env::var("NEUROX_MAX_TOOL_ITERATIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
}

/// Max wall-clock time to wait for a SINGLE session-agent stream round
/// trip (one `process` / `tool_result` JSON-RPC call to the subprocess)
/// before giving up and closing the turn.
///
/// EP-2026-09-05 (stuck-turn fix): the session-agent path had NO
/// timeout. If the subprocess stalled mid-LLM-call (e.g. the upstream
/// stream trickles keep-alive bytes without ever finishing, so the
/// subprocess never writes its final JSON-RPC `result` line), the
/// daemon blocked forever on `read_line`, never emitted `Event::Done`,
/// and the SSE never sent `[DONE]` — the turn stayed "streaming"
/// indefinitely (input frozen with a cancel button). This bounds the
/// wait so a hung round trip surfaces as an error and closes the turn.
///
/// Configurable via `NEUROX_SESSION_STREAM_TIMEOUT_SECS`; defaults to
/// 180s (generous — a long tool-augmented reply can legitimately take
/// a couple of minutes, but not forever).
pub fn session_stream_timeout_secs() -> u64 {
    std::env::var("NEUROX_SESSION_STREAM_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &u64| n > 0)
        .unwrap_or(180)
}

/// Agent IDs that are built into the daemon (no subprocess) and
/// therefore not present in the registry. These still appear in
/// `GET /v1/agents` listings and accept session creation.
///
/// Kept for backward compatibility with external code that referenced
/// the legacy `default` constant — actual routing is now driven
/// exclusively by `session_agents` config and `POST /v1/agents/in_process`.
pub const DEFAULT_AGENT_ID: &str = "default";

impl AppState {
    /// Workspace efectivo de una sesión, como `WorkspaceScope` listo para
    /// el `ExecuteContext`.
    ///
    /// Es el punto donde el aislamiento se decide: la sesión trae el id, el
    /// id trae el root y el sandbox, y el tool usa eso en vez de su par
    /// propio. Sin sesión, sin workspaces, o con un id que no resuelve, cae
    /// al sandbox global con el root global: el comportamiento de antes, y
    /// un id mal escrito no deja al agente sin permisos.
    pub async fn scope_for_session(
        &self,
        session_id: uuid::Uuid,
    ) -> tools_engine::WorkspaceScope {
        let global_root = self.workspace.workspace_root.clone();
        let Some(layers) = self.workspaces.as_ref() else {
            return tools_engine::WorkspaceScope {
                id: None,
                root: global_root,
                sandbox: self.workspace.sandbox.clone(),
            };
        };
        let workspace_id = self
            .lifecycle
            .session
            .get_workspace_id(session_id)
            .await
            .ok()
            .flatten();
        crate::workspaces::resolve_scope(
            &layers.store,
            &layers.registry,
            workspace_id.as_deref(),
            &global_root,
        )
        .await
    }

    /// Workspace efectivo de un agente persistente, por su `workspace_id`.
    ///
    /// Los agentes con session usan `scope_for_session`, que gana: es el
    /// override por sesión. Este es el default que aplica a los agentes
    /// persistentes y a las sesiones que no elegieron nada.
    pub async fn scope_for_agent(&self, agent_id: &str) -> tools_engine::WorkspaceScope {
        let global_root = self.workspace.workspace_root.clone();
        let Some(layers) = self.workspaces.as_ref() else {
            return tools_engine::WorkspaceScope {
                id: None,
                root: global_root,
                sandbox: self.workspace.sandbox.clone(),
            };
        };
        // El default del agente: su `workspace_id` en el spec. Si el agente
        // no declara uno, se usa el workspace marcado `is_default`, que es
        // el "entorno por defecto" del operator. Sin ninguno de los dos, el
        // global.
        let del_spec = self
            .lifecycle
            .session_agents
            .spec(agent_id)
            .await
            .and_then(|s| s.workspace_id);
        let del_agente = match del_spec {
            Some(id) => Some(id),
            None => layers
                .store
                .list()
                .await
                .ok()
                .and_then(|list| list.into_iter().find(|w| w.is_default).map(|w| w.id)),
        };
        crate::workspaces::resolve_scope(
            &layers.store,
            &layers.registry,
            del_agente.as_deref(),
            &global_root,
        )
        .await
    }

    /// Returns true if `agent_id` is something we can dispatch to:
    /// the in-process default, any other configured in-process
    /// agent (EP-2026-08-15 multi-agent), a registered persistent
    /// agent, a session-scoped agent (per-session subprocess), or an
    /// ephemeral template. Use this for session/agent validation.
    pub async fn is_known_agent(&self, agent_id: &str) -> bool {
        // EP-2026-08-15: accept any in-process spec the daemon is
        // configured for. We don't yet spawn them in parallel —
        // daemon still has ONE live in-process agent, but now the
        // /v1/agents listing and routing queries the full list.
        if self
            .config
            .resolved_in_process()
            .iter()
            .any(|spec| spec.id == agent_id)
        {
            return true;
        }
        if self.lifecycle.session_agents.is_session_agent(agent_id).await {
            return true;
        }
        let persistent = self.lifecycle.registry.list_persistent().await;
        if persistent.iter().any(|p| p.id == agent_id) {
            return true;
        }
        let ephemeral = self.lifecycle.registry.list_ephemeral_templates().await;
        if ephemeral.iter().any(|e| e.id == agent_id) {
            return true;
        }
        false
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.event_tx.subscribe()
    }

    /// Extract cumulative output tokens from an agent's response. The
    /// agent returns `{ tokens_out: <u64>, ... }` at the end of every
    /// dispatch. `None` when the agent doesn't track tokens (e.g. the
    /// mock agent used in tests).
    fn extract_tokens_used(result: &Option<serde_json::Value>) -> Option<u64> {
        result
            .as_ref()
            .and_then(|v| v.get("tokens_out"))
            .and_then(|v| v.as_u64())
    }

    /// Dispatch a request to a session-bound subprocess. Used by the
    /// `SessionAgentPool` path — each session has its own dedicated process
    /// and `protocol.stream` is the round-trip that pulls the response
    /// (with streaming `Content` events) back through the global event bus.
    ///
    /// This handles the full request lifecycle: forwards events to the
    /// broadcast channel, processes any tool calls through the local
    /// `ToolRegistry`, and recurses back through the agent on tool results
    /// (capped at `MAX_TOOL_ITERATIONS`).
    async fn dispatch_to_session_agent(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        method: &str,
        session_id: Uuid,
        params: serde_json::Value,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let _agent_id = agent.agent_id.clone();
        let (cancel_id, cancel) = self.lifecycle.tasks.create(session_id).await;

        // Touch last_active so the idle sweeper doesn't kill us mid-flight.
        agent.touch();

        // RAII: the inflight counter is bumped now and released on
        // every return path (including tool_call recursion).
        let _inflight = crate::session_agents::InflightGuard::new(agent.inflight.clone());

        // EP-2026-08-15 (live-switch history injection): the very first
        // dispatch on a freshly-spawned subprocess needs to seed its
        // working memory with the session's prior history (read from
        // the daemon's own DB). This covers two cases:
        //   1. Brand-new session: history is empty → no-op.
        //   2. Live agent switch (user changed agents in the UI): the
        //      OLD subprocess was killed by `start_for_session` and the
        //      NEW one starts with empty memory. Without this seed the
        //      LLM would have no context of the previous conversation.
        // The flag is `false` on creation and set to `true` after a
        // successful seed; subsequent dispatches skip the DB read.
        if !agent.history_seeded.load(std::sync::atomic::Ordering::Acquire) {
            let history = self
                .lifecycle
                .session
                .get_messages(session_id)
                .await
                .unwrap_or_default();
            // EP-2026-08-15 (live-switch history injection): exclude the
            // most recent message from the seed. The daemon logs the
            // current dispatch's user message BEFORE dispatch (so the
            // `/v1/sessions/:id/messages` history shows it), but the
            // subprocess's `handle_process` will add the same text to
            // its own WorkingMemory on the `process` call. Seeding it
            // twice would duplicate the entry in the LLM context.
            let n_seed = history.len().saturating_sub(1);
            let seed_messages: Vec<serde_json::Value> = history
                .iter()
                .take(n_seed)
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                })
                .collect();
            let seed_params = serde_json::json!({
                "messages": seed_messages,
            });
            let seed_req = crate::protocols::AgentRequest {
                session_id,
                method: "seed_history".to_string(),
                params: seed_params,
            };
            match agent.protocol.call(seed_req, cancel.clone()).await {
                Ok(resp) => {
                    agent
                        .history_seeded
                        .store(true, std::sync::atomic::Ordering::Release);
                    eprintln!(
                        "[router] seeded history for session={} agent={} → {} messages (DB had {}, excluding current dispatch msg)",
                        session_id,
                        agent.agent_id,
                        n_seed,
                        history.len()
                    );
                    let _ = resp; // seed_history returns {seeded:N}, we don't need it
                }
                Err(e) => {
                    // Non-fatal: log and proceed. The dispatch will still
                    // go through; the new agent just has no context.
                    tracing::warn!(
                        session_id = %session_id,
                        agent_id = %agent.agent_id,
                        error = %e,
                        "EP-2026-08-15: seed_history failed; dispatch proceeds without context"
                    );
                }
            }
        }

        let result = self
            .stream_session_agent(agent.clone(), method, session_id, params.clone(), cancel.clone())
            .await;

        // Capture real tokens from the agent's response (the agent's
        // own WorkingMemory tracks cumulative output tokens; we just
        // persist whatever the agent emits).
        if let Ok(ref resp) = result {
            if let Some(tokens) = Self::extract_tokens_used(&resp.result) {
                if let Err(e) = self.lifecycle.session.set_tokens_used(session_id, tokens).await {
                    tracing::warn!(session_id = %session_id, error = %e, "tokens_used update failed");
                }
            }
        }

        self.lifecycle.tasks.cleanup(session_id, cancel_id).await;

        let response = match result {
            Ok(resp) => resp,
            Err(e) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: e.to_string(),
                });
                return Err(e);
            }
        };

        // If the agent returned tool_call(s), run them through the local
        // ToolRegistry and loop results back through the agent (mirrors
        // the persistent-agent path).
        //
        // D1 fix: dispatch ALL parallel calls (tool_calls is an array).
        // The previous implementation took only tool_calls[0] and
        // silently dropped the rest, which broke the standard chat-API
        // contract when the model emitted parallel function calls.
        // The dispatch is sequential so the SSE stream preserves the
        // tool_call/tool_result pairing order expected by both clients.
        if let Some(ref result_obj) = response.result {
            let tool_calls: Vec<serde_json::Value> = result_obj
                .get("tool_calls")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            // Also fall back to the legacy single `tool_call` field.
            let calls: Vec<serde_json::Value> = if !tool_calls.is_empty() {
                tool_calls
            } else if let Some(single) = result_obj.get("tool_call").cloned() {
                vec![single]
            } else {
                Vec::new()
            };

            if !calls.is_empty() {
                let (_cancel_id, cancel) = self.lifecycle.tasks.create(session_id).await;
                return Box::pin(self.handle_session_tool_calls_batch(
                    agent,
                    session_id,
                    calls,
                    &params,
                    cancel,
                    1,
                ))
                .await;
            }
        }

        // Emit a final Done event with the agent's text. The streamed
        // per-chunk Content events are already in the broadcast bus.
        if let Some(ref result_obj) = response.result {
            if let Some(text) = result_obj.get("text").and_then(|v| v.as_str()) {
                let _ = self.events.event_tx.send(Event::Done {
                    session_id,
                    text: text.to_string(),
                });
            }
        }
        Ok(response.result)
    }

    /// Perform a single JSON-RPC stream call against the session-bound
    /// subprocess. Spawns a forwarder task that pipes the agent's
    /// notifications into the global event bus.
    async fn stream_session_agent(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        method: &str,
        session_id: Uuid,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<AgentResponse> {
        let (tx, mut rx) = mpsc::channel::<Event>(64);
        let event_tx = self.events.event_tx.clone();
        let events_layer = self.events.clone();
        let cancel_for_forward = cancel.clone();
        let forward = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    () = cancel_for_forward.cancelled() => break,
                    evt = rx.recv() => {
                        match evt {
                            Some(mut event) => {
                                // EP-2026-09-05 (stream seq): stamp the
                                // per-session sequence right before the
                                // event hits the bus. Content/Thinking
                                // are born in the protocol stream with a
                                // placeholder seq (0); assign the real
                                // monotonic value here so ordering is
                                // consistent for every subscriber.
                                if let Event::Content { session_id, seq, .. }
                                | Event::Thinking { session_id, seq, .. } = &mut event
                                {
                                    *seq = events_layer.next_seq(*session_id);
                                }
                                if event_tx.send(event).is_err() { break; }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let req = AgentRequest {
            session_id,
            method: method.to_string(),
            params,
        };
        // EP-2026-09-05 (stuck-turn fix): bound the round trip. Without
        // this, a stalled subprocess (LLM stream that never completes)
        // blocks `protocol.stream` -> `read_line` forever, so the turn
        // never closes. On timeout we cancel the token (aborts the
        // subprocess-side wait), surface an Error + Done on the bus so
        // BOTH transports close the turn (SSE `[DONE]`, WS stream-end),
        // and return Err so the caller's tool-loop unwinds.
        let timeout = std::time::Duration::from_secs(session_stream_timeout_secs());
        let stream_fut = agent.protocol.stream(req, cancel.clone(), tx.clone());
        let result = match tokio::time::timeout(timeout, stream_fut).await {
            Ok(r) => r,
            Err(_elapsed) => {
                warn!(
                    session_id = %session_id,
                    timeout_secs = timeout.as_secs(),
                    "session-agent stream timed out; closing turn"
                );
                // Abort the subprocess-side wait so the pipe is freed.
                cancel.cancel();
                // Close the turn for every subscriber. These go through
                // the forwarder below (still draining `rx`) and then the
                // global bus.
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: format!(
                        "El agente no respondió a tiempo ({}s). El turno se cerró.",
                        timeout.as_secs()
                    ),
                });
                let _ = self.events.event_tx.send(Event::Done {
                    session_id,
                    text: String::new(),
                });
                Err(anyhow::anyhow!(
                    "session-agent stream timed out after {}s",
                    timeout.as_secs()
                ))
            }
        };
        drop(tx);
        let _ = forward.await;
        result
    }

    /// Run a tool_call emitted by the session-bound agent: emit ToolCall
    /// event, gate on approval, execute through the tool registry, emit
    /// ToolResult, then send the result back to the agent (loop up to
    /// `MAX_TOOL_ITERATIONS`).
    ///
    /// EP-0003 Tier 2: soft cap at `MAX_TOOL_ITERATIONS`. Instead of
    /// hard-failing, the daemon injects a synthetic user message
    /// asking the agent to summarize its progress and either ask the
    /// user for next steps OR take a definitive action — NO more tool
    /// calls. The agent gets ONE more iteration to respond with a
    /// final assistant message (text only, no tool_call). If the
    /// agent still emits a tool_call in that final iteration (LLM is
    /// stubborn sometimes), the daemon hard-stops with
    /// `AgentError::IterationLimit`.
    ///
    /// Behavior can be reverted to the old hard-fail by setting
    /// `NEUROX_STRICT_ITERATION_LIMIT=true` (env var, scoped to the
    /// `agent` subprocess — Tier 3 will wire this up).
    async fn handle_session_tool_call(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        tool_call_value: serde_json::Value,
        original_params: &serde_json::Value,
        cancel: CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if iteration >= max_tool_iterations() {
            // Soft cap: inject synthetic message asking the agent to
            // wrap up. The events from this final dispatch flow
            // through the same forwarder, so the user sees the agent's
            // summary streamed in real time.
            let synthetic_text = format!(
                "[system] You've iterated {iteration} times on this request. \
                 STOP making tool calls. Summarize what you've found so far \
                 and ask the user for next steps, OR take a definitive action. \
                 Your next response MUST be a final assistant message with no \
                 tool_calls — otherwise this loop will hard-stop with \
                 IterationLimit."
            );
            // We deliberately send `tools: []` so the LLM is
            // structurally prevented from emitting tool_calls. Even
            // if the model is stubborn, the parse will surface
            // tool_calls as an empty array, not a runaway loop.
            let synthetic_params = serde_json::json!({
                "params": { "text": synthetic_text, "tools": [] },
            });

            let result = self
                .dispatch_to_session_agent(
                    agent.clone(),
                    "process", // soft-cap is always a `process` call
                    session_id,
                    synthetic_params,
                )
                .await;

            match result {
                Ok(Some(_)) | Ok(None) => {
                    // Agent responded (probably a final text message,
                    // since tools was empty). Events already streamed
                    // through the forwarder. Emit Done to close the
                    // SSE stream cleanly.
                }
                Err(e) => {
                    // Dispatch failed (rare — e.g. subprocess died).
                    // Surface as error and close stream.
                    let _ = self.events.event_tx.send(Event::Error {
                        session_id: Some(session_id),
                        message: format!(
                            "soft-cap dispatch failed: {e}"
                        ),
                    });
                }
            }
            let _ = self.events.event_tx.send(Event::Done {
                session_id,
                text: String::new(),
            });
            return Ok(None);
        }
        let call: ToolCall = serde_json::from_value(tool_call_value.clone())
            .map_err(|e| anyhow::anyhow!("invalid tool_call: {e}"))?;

        let tool = if let Some(t) = self.engine.tools.get(&call.name) {
            t
        } else {
            // EP-0024-UX: when the LLM hallucinates a tool_call to a
            // tool that isn't registered, the SSE forwarder (in
            // router/http.rs) only sees `Event::ToolCall` and never
            // sees a corresponding `Event::ToolResult` because we
            // used to `return Ok(None)` here. That left the tool
            // entry in the chat transcript with its spinner running
            // forever. Emit a synthetic `ToolResult` with the error
            // so the timeline gets closed and the user sees the
            // "unknown tool" message in place of a tool output.
            //
            // Note: we deliberately do NOT send `Event::Error` here
            // because the SSE forwarder closes the stream with
            // `[DONE]` on `Event::Error` (see router/http.rs), which
            // would race against the `ToolResult` we just emitted.
            // The result of the tool is the user-facing surface for
            // this case.
            let err = format!("[error] unknown tool: {}", call.name);
            let _ = self.events.event_tx.send(Event::ToolResult {
                session_id,
                tool: call.name.clone(),
                result: err,
                iteration,
                seq: self.events.next_seq(session_id),
            });
            return Ok(None);
        };

        let spec = tool.spec();

        let _ = self.events.event_tx.send(Event::ToolCall {
            session_id,
            tool: call.name.clone(),
            args: call.args.clone(),
            iteration,
            seq: self.events.next_seq(session_id),
        });

        // Per-agent approval override: if the spec defines an explicit list,
        // only the named tools require approval (an empty list disables
        // approval entirely for this agent). Otherwise fall back to the
        // tool's own flag from llmd (backwards-compatible default).
        let needs_approval = match &agent.spec.requires_approval {
            Some(list) => list.contains(&call.name),
            None => spec.requires_approval,
        };
        if needs_approval {
            let approval_id = uuid::Uuid::new_v4();
            let request = crate::approval::ApprovalRequest {
                id: approval_id,
                session_id,
                tool: call.name.clone(),
                args: call.args.clone(),
                reason: Some("tool requires approval".into()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.event_tx.send(Event::ApprovalRequest {
                request: request.clone(),
            });
            let (_id, decision) = self.lifecycle
                .approvals
                .request_with_id(
                    approval_id,
                    session_id,
                    call.name.clone(),
                    call.args.clone(),
                    request.reason.clone(),
                )
                .await;
            let _ = self.events.event_tx.send(Event::ApprovalResolved {
                approval_id,
                session_id,
                tool: call.name.clone(),
                decision: match decision {
                    crate::approval::ApprovalDecision::Approve => "approve".into(),
                    crate::approval::ApprovalDecision::Deny => "deny".into(),
                },
            });
            if matches!(decision, crate::approval::ApprovalDecision::Deny) {
                return Box::pin(self.dispatch_session_tool_result(
                    agent,
                    session_id,
                    &call,
                    "[denied by user]",
                    original_params,
                    cancel,
                    iteration,
                ))
                .await;
            }
        }

        let exec_start = std::time::Instant::now();
        let ctx = tools_engine::ExecuteContext {
            agent_id: agent.agent_id.clone(),
            workspace: Some(self.scope_for_session(session_id).await),
            cancel: None,
            http_client: None,
        };
        let result = tool.execute(&ctx, call.args.clone()).await;
        let exec_duration = exec_start.elapsed();
        let outcome = if result.is_ok() { "ok" } else { "err" };
        tracing::info!(
            agent_id = %agent.agent_id,
            session_id = %session_id,
            tool = %call.name,
            outcome = %outcome,
            duration_ms = exec_duration.as_millis() as u64,
            "tool_executed"
        );
        let output = match result {
            Ok(s) => s,
            Err(e) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: format!("tool '{}' error: {}", call.name, e),
                });
                format!("[error] {e}")
            }
        };

        let _ = self.events.event_tx.send(Event::ToolResult {
            session_id,
            tool: call.name.clone(),
            result: output.clone(),
            iteration,
            seq: self.events.next_seq(session_id),
        });

        self.dispatch_session_tool_result(
            agent,
            session_id,
            &call,
            &output,
            original_params,
            cancel,
            iteration,
        )
        .await
    }

    /// Send a tool result back to the session-bound agent and recurse on
    /// any further tool_calls it returns.
    async fn dispatch_session_tool_result(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        call: &ToolCall,
        output: &str,
        original_params: &serde_json::Value,
        cancel: CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let params = serde_json::json!({
            "call_id": call.id,
            "result": output,
        });
        let resp = self
            .stream_session_agent(
                agent.clone(),
                "tool_result",
                session_id,
                params,
                cancel.clone(),
            )
            .await?;

        if !resp.ok {
            let _ = self.events.event_tx.send(Event::Error {
                session_id: Some(session_id),
                message: resp
                    .error
                    .unwrap_or_else(|| "agent returned error after tool result".into()),
            });
            return Ok(None);
        }

        // D1 fix: take ALL parallel tool_calls, not just [0]. If the
        // LLM emitted a batch in the next round, dispatch each one
        // sequentially (preserving order in the SSE stream).
        let next_calls: Vec<serde_json::Value> = if let Some(ref r) = resp.result {
            if let Some(arr) = r.get("tool_calls").and_then(|v| v.as_array()) {
                arr.clone()
            } else if let Some(single) = r.get("tool_call").cloned() {
                vec![single]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if !next_calls.is_empty() {
            return Box::pin(self.handle_session_tool_calls_batch(
                agent,
                session_id,
                next_calls,
                original_params,
                cancel,
                iteration + 1,
            ))
            .await;
        }

        let _ = self.events.event_tx.send(Event::Done {
            session_id,
            text: serde_json::to_string(&resp.result)?,
        });
        Ok(resp.result)
    }

    /// D1 fix: dispatch a batch of parallel tool_calls sequentially.
    /// Each call emits its own ToolCall + ToolResult pair; the SSE
    /// forwarder preserves order, so the client sees a deterministic
    /// call1→result1→call2→result2→... chain.
    ///
    /// Tool execution is synchronous within this function. Parallel
    /// execution (tokio::join!) would be faster for I/O-bound tools
    /// but complicates ordering and the Event::Error recovery path.
    /// Sequential is safer and good enough until we have evidence of a
    /// real perf bottleneck.
    async fn handle_session_tool_calls_batch(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        calls: Vec<serde_json::Value>,
        original_params: &serde_json::Value,
        cancel: CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let mut last_result: Option<serde_json::Value> = None;
        for call in calls {
            let r = self
                .handle_session_tool_call(
                    agent.clone(),
                    session_id,
                    call,
                    original_params,
                    cancel.clone(),
                    iteration,
                )
                .await?;
            if r.is_some() {
                last_result = r;
            }
        }
        Ok(last_result)
    }

    pub async fn dispatch_to_agent(
        &self,
        agent_id: &str,
        method: &str,
        session_id: Uuid,
        params: serde_json::Value,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        // ─── SessionAgentPool: per-session subprocess (real parallelism) ─────
        // EP-2026-08-15 (live-switch fix): respect the body's `agent_id`
        // over the session's existing binding. If the session is bound to
        // a *different* agent (because the user switched agents in the UI),
        // start a fresh subprocess for `agent_id` — `start_for_session`
        // kills the old subprocess and binds the new one to the same
        // session_id. Net effect: same session_id, same chat history on
        // the frontend, new agent handles the next message. The new
        // subprocess has empty working memory (does NOT inherit from
        // the killed one) — that's the trade-off the user accepted when
        // picking "Live switch: mismo session_id, agente cambia al vuelo".
        if self.lifecycle.session_agents.is_session_agent(agent_id).await {
            // Fast path: session already on the requested agent — just dispatch.
            // EP-2026-08-31: `get_or_respawn` replaces the cached
            // SessionAgent if its subprocess died (manual kill, OOM,
            // crash). Without this the dispatch tries to read from a
            // dead pipe and hangs for the full HTTP timeout.
            if let Some(existing) = self.lifecycle.session_agents.get_or_respawn(session_id).await {
                if existing.agent_id == agent_id {
                    return self
                        .dispatch_to_session_agent(existing, method, session_id, params)
                        .await;
                }
                // Live-switch path: agent_id differs. Tear down the
                // respawned one and start fresh for the new agent_id.
                let _ = self.lifecycle.session_agents.stop(session_id).await;
            }
            // Slow path: bind (or rebind) the session to the requested agent.
            // `start_for_session` handles the "kill old, insert new" atomically
            // (it locks `self.agents` for the insert and calls `shutdown()` on
            // the replaced subprocess).
            let agent = self
                .lifecycle
                .session_agents
                .start_for_session(session_id, agent_id)
                .await?;
            return self
                .dispatch_to_session_agent(agent, method, session_id, params)
                .await;
        }

        // ─── In-process agent: direct call to llmd (no subprocess) ─────────────
        // EP-2026-08-15: accept ANY in_process agent listed in config
        // (multi-agent setup). `agent` is the legacy single-id slot and is
        // accepted via the resolved_in_process() helper.
        if method == "process"
            && self
                .config
                .resolved_in_process()
                .iter()
                .any(|spec| spec.id == agent_id)
        {
            let text = params
                .get("params")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .or_else(|| params.get("text").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();

            let (cancel_id, cancel) = self.lifecycle.tasks.create(session_id).await;
            let _ = cancel; // currently unused; reserved for Phase 2-b cancel.

            // EP-2026-08-15 (Phase 2-b): chat dispatch through the
            // subprocess registered for this agent_id (admin / user /
            // agent / any future runtime-registered agent). Each
            // subprocess reads JSON-RPC from its stdin/stdout and runs
            // its own LLM call directly. Different agents run in
            // parallel because each has its own process; calls to the
            // SAME agent serialize via a per-subprocess Mutex inside
            // session_agents.persistent (EP-0004 wave 3).
            //
            // The legacy path (ad-hoc `llmd_client.chat(...)`) is no
            // longer used here.
            let tools = params
                .get("params")
                .and_then(|p| p.get("tools"))
                .cloned()
                .unwrap_or(serde_json::json!([]));
            let chat_res = self
                .lifecycle
                .session_agents
                .send_chat_persistent(agent_id, &text, session_id, tools)
                .await;
            self.lifecycle.tasks.cleanup(session_id, cancel_id).await;

            return match chat_res {
                Ok(outcome) => Ok(Some(serde_json::json!({
                    "text": outcome.text,
                    "session_id": session_id.to_string(),
                }))),
                Err(e) => {
                    let _ = self.events.event_tx.send(Event::Error {
                        session_id: Some(session_id),
                        message: e.to_string(),
                    });
                    Err(anyhow::anyhow!(e))
                }
            };
        }

        // Clone params for potential tool_call reuse
        let params_for_tool = params.clone();

        // Acquire cancellation token for this session up front — both
        // persistent and ephemeral paths need it.
        let (cancel_id, cancel) = self.lifecycle.tasks.create(session_id).await;

        // If the agent is already running (persistent), use it directly.
        // Otherwise look for an ephemeral template and spawn a one-shot agent.
        let protocol = if let Some(p) = self.lifecycle.supervisor.get_protocol(agent_id).await {
            p
        } else if let Some(template) = self.lifecycle.registry.get_ephemeral_template(agent_id).await {
            // Spawn ephemeral agent, dispatch the request, and shut it down.
            let req = AgentRequest {
                session_id,
                method: method.to_string(),
                params: params.clone(),
            };
            let cancel_for_eph = cancel.clone();
            let (tx, mut rx) = mpsc::channel::<Event>(64);
            let tx_for_drop = tx.clone(); // kept alive to close the channel after spawn
            let event_tx = self.events.event_tx.clone();
            let events_layer = self.events.clone();
            let cancel_for_forward = cancel.clone();
            // Forward spawned-agent events to the broadcast channel
            let forward = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        biased;
                        () = cancel_for_forward.cancelled() => break,
                        evt = rx.recv() => {
                            match evt {
                                Some(mut event) => {
                                    // EP-2026-09-05 (stream seq): stamp
                                    // the per-session sequence before
                                    // broadcasting (see stream_session_agent).
                                    if let Event::Content { session_id, seq, .. }
                                    | Event::Thinking { session_id, seq, .. } = &mut event
                                    {
                                        *seq = events_layer.next_seq(*session_id);
                                    }
                                    if event_tx.send(event).is_err() {
                                        break;
                                    }
                                }
                                None => break,
                            }
                        }
                    }
                }
            });

            let result = self
                .lifecycle
                .spawner
                .spawn(
                    agent_id,
                    template,
                    session_id,
                    tx,
                    move |proto, tx| async move {
                        proto.stream(req, cancel_for_eph, tx).await.map(|_| ())
                    },
                )
                .await;

            // Drop the channel so forward exits once the spawned task finishes.
            drop(tx_for_drop);
            let _ = forward.await;

            // cleanup handled by the spawned task inside spawner; cleanup our token too
            self.lifecycle.tasks.cleanup(session_id, cancel_id).await;
            match result {
                Ok(_) => {
                    // Emit a Done event with empty text — the spawned task already
                    // emitted content/done events through its protocol stream.
                    let _ = self.events.event_tx.send(Event::Done {
                        session_id,
                        text: String::new(),
                    });
                    return Ok(None);
                }
                Err(e) => {
                    let _ = self.events.event_tx.send(Event::Error {
                        session_id: Some(session_id),
                        message: format!("spawn failed: {e}"),
                    });
                    return Ok(None);
                }
            }
        } else {
            let _ = self.events.event_tx.send(Event::Error {
                session_id: Some(session_id),
                message: format!("agent not running and no ephemeral template: {agent_id}"),
            });
            self.lifecycle.tasks.cleanup(session_id, cancel_id).await;
            return Ok(None);
        };

        let (tx, mut rx) = mpsc::channel::<Event>(64);
        let req = AgentRequest {
            session_id,
            method: method.to_string(),
            params,
        };

        let cancel_for_stream = cancel.clone();
        let cancel_for_forward = cancel.clone();

        let event_tx = self.events.event_tx.clone();
        let forward = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    () = cancel_for_forward.cancelled() => break,
                    evt = rx.recv() => {
                        match evt {
                            Some(event) => {
                                if event_tx.send(event).is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let result = protocol.stream(req, cancel_for_stream, tx.clone()).await;
        drop(tx);
        let _ = forward.await;

        // Clean up the token once work is done
        self.lifecycle.tasks.cleanup(session_id, cancel_id).await;

        match result {
            Ok(resp) if resp.ok => {
                // Check if the response contains tool_calls. If so, dispatch them
                // through the local ToolRegistry, then loop the results back
                // to the agent (max iterations to avoid infinite loops).
                if let Some(result_obj) = resp.result.as_ref() {
                    // Try tool_calls array first, fallback to single tool_call
                    let tool_calls: Vec<serde_json::Value> = result_obj
                        .get("tool_calls")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    if !tool_calls.is_empty() {
                        // D1 fix: dispatch ALL parallel tool_calls
                        // sequentially, not just [0].
                        return self
                            .handle_tool_calls_batch(
                                agent_id,
                                session_id,
                                tool_calls,
                                &params_for_tool,
                                cancel.clone(),
                                1,
                            )
                            .await;
                    } else if let Some(tool_call_value) = result_obj.get("tool_call").cloned() {
                        return self
                            .handle_tool_call(
                                agent_id,
                                session_id,
                                tool_call_value,
                                &params_for_tool,
                                cancel.clone(),
                                1,
                            )
                            .await;
                    }
                }
                let _ = self.events.event_tx.send(Event::Done {
                    session_id,
                    text: serde_json::to_string(&resp.result)?,
                });
                Ok(resp.result)
            }
            Ok(resp) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: resp.error.unwrap_or_else(|| "unknown error".to_string()),
                });
                Ok(None)
            }
            Err(e) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: e.to_string(),
                });
                Err(e)
            }
        }
    }

    /// Handle a `tool_call` emitted by the agent:
    /// 1. Validate the tool exists in the `ToolRegistry`
    /// 2. Check if approval is required (gated on `requires_approval` field)
    /// 3. Emit `ToolCall` event, await approval if needed
    /// 4. Execute the tool, emit `ToolResult` event
    /// 5. Re-call the agent with the result (up to MAX_TOOL_ITERATIONS)
    async fn handle_tool_call(
        &self,
        agent_id: &str,
        session_id: Uuid,
        tool_call_value: serde_json::Value,
        original_params: &serde_json::Value,
        cancel: tokio_util::sync::CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if iteration >= max_tool_iterations() {
            let _ = self.events.event_tx.send(Event::Error {
                session_id: Some(session_id),
                message: format!("tool_call loop exceeded max iterations ({})", max_tool_iterations()),
            });
            return Ok(None);
        }
        let call: ToolCall = serde_json::from_value(tool_call_value.clone())
            .map_err(|e| anyhow::anyhow!("invalid tool_call: {e}"))?;

        let tool = if let Some(t) = self.engine.tools.get(&call.name) {
            t
        } else {
            let _ = self.events.event_tx.send(Event::Error {
                session_id: Some(session_id),
                message: format!("unknown tool: {}", call.name),
            });
            return Ok(None);
        };

        let spec = tool.spec();

        // Emit the ToolCall event before execution
        let _ = self.events.event_tx.send(Event::ToolCall {
            session_id,
            tool: call.name.clone(),
            args: call.args.clone(),
            iteration,
            seq: self.events.next_seq(session_id),
        });

        // Check approval
        if spec.requires_approval {
            // Generate approval ID upfront so event and manager use the same one
            let approval_id = uuid::Uuid::new_v4();
            let request = crate::approval::ApprovalRequest {
                id: approval_id,
                session_id,
                tool: call.name.clone(),
                args: call.args.clone(),
                reason: Some("tool requires approval".into()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            // Emit event BEFORE blocking (so TUI sees it immediately)
            let _ = self.events.event_tx.send(Event::ApprovalRequest {
                request: request.clone(),
            });
            // Now block waiting for user response (uses same ID)
            let (_id, decision) = self.lifecycle
                .approvals
                .request_with_id(
                    approval_id,
                    session_id,
                    call.name.clone(),
                    call.args.clone(),
                    request.reason.clone(),
                )
                .await;
            let _ = self.events.event_tx.send(Event::ApprovalResolved {
                approval_id,
                session_id,
                tool: call.name.clone(),
                decision: match decision {
                    crate::approval::ApprovalDecision::Approve => "approve".into(),
                    crate::approval::ApprovalDecision::Deny => "deny".into(),
                },
            });
            if matches!(decision, crate::approval::ApprovalDecision::Deny) {
                return self
                    .dispatch_tool_result(
                        agent_id,
                        session_id,
                        &call,
                        "[denied by user]",
                        original_params,
                        cancel,
                        iteration,
                    )
                    .await;
            }
        }

        // Execute (EP-0019-08: log structured tool invocation)
        let exec_start = std::time::Instant::now();
        let ctx = tools_engine::ExecuteContext {
            agent_id: agent_id.to_string(),
            workspace: Some(self.scope_for_agent(agent_id).await),
            cancel: None,
            http_client: None,
        };
        let result = tool.execute(&ctx, call.args.clone()).await;
        let exec_duration = exec_start.elapsed();
        let outcome = if result.is_ok() { "ok" } else { "err" };
        tracing::info!(
            agent_id = %agent_id,
            session_id = %session_id,
            tool = %call.name,
            outcome = %outcome,
            duration_ms = exec_duration.as_millis() as u64,
            "tool_executed"
        );
        let output = match result {
            Ok(s) => s,
            Err(e) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: format!("tool '{}' error: {}", call.name, e),
                });
                format!("[error] {e}")
            }
        };

        // Emit ToolResult
        let _ = self.events.event_tx.send(Event::ToolResult {
            session_id,
            tool: call.name.clone(),
            result: output.clone(),
            iteration,
            seq: self.events.next_seq(session_id),
        });

        self.dispatch_tool_result(
            agent_id,
            session_id,
            &call,
            &output,
            original_params,
            cancel,
            iteration,
        )
        .await
    }

    /// Send a tool result back to the agent and return its final response.
    async fn dispatch_tool_result(
        &self,
        agent_id: &str,
        session_id: Uuid,
        call: &ToolCall,
        output: &str,
        _original_params: &serde_json::Value,
        cancel: tokio_util::sync::CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let protocol = match self.lifecycle.supervisor.get_protocol(agent_id).await {
            Some(p) => p,
            None => return Ok(None),
        };

        let params = serde_json::json!({
            "call_id": call.id,
            "result": output,
        });
        let req = AgentRequest {
            session_id,
            method: "tool_result".into(),
            params,
        };

        let (tx, mut rx) = mpsc::channel::<Event>(64);
        let event_tx = self.events.event_tx.clone();
        let cancel_for_forward = cancel.clone();
        let forward = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    () = cancel_for_forward.cancelled() => break,
                    evt = rx.recv() => {
                        match evt {
                            Some(event) => { if event_tx.send(event).is_err() { break; } }
                            None => break,
                        }
                    }
                }
            }
        });

        let result = protocol.stream(req, cancel.clone(), tx.clone()).await;
        drop(tx);
        let _ = forward.await;

        match result {
            Ok(resp) if resp.ok => {
                // D1 fix: dispatch ALL parallel tool_calls, not just
                // tool_calls[0]. The persistent-agent path mirrors the
                // session-agent path fix.
                let next_calls: Vec<serde_json::Value> =
                    if let Some(ref r) = resp.result {
                        if let Some(arr) = r.get("tool_calls").and_then(|v| v.as_array()) {
                            arr.clone()
                        } else if let Some(single) = r.get("tool_call").cloned() {
                            vec![single]
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };
                if !next_calls.is_empty() {
                    return Box::pin(self.handle_tool_calls_batch(
                        agent_id,
                        session_id,
                        next_calls,
                        _original_params,
                        cancel.clone(),
                        iteration + 1,
                    ))
                    .await;
                }
                let _ = self.events.event_tx.send(Event::Done {
                    session_id,
                    text: serde_json::to_string(&resp.result)?,
                });
                Ok(resp.result)
            }
            Ok(_) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: "agent returned error after tool result".into(),
                });
                Ok(None)
            }
            Err(e) => {
                let _ = self.events.event_tx.send(Event::Error {
                    session_id: Some(session_id),
                    message: e.to_string(),
                });
                Err(e)
            }
        }
    }

    /// D1 fix: persistent-agent variant of the parallel batch handler.
    async fn handle_tool_calls_batch(
        &self,
        agent_id: &str,
        session_id: Uuid,
        calls: Vec<serde_json::Value>,
        original_params: &serde_json::Value,
        cancel: tokio_util::sync::CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let mut last_result: Option<serde_json::Value> = None;
        for call in calls {
            let r = self
                .handle_tool_call(
                    agent_id,
                    session_id,
                    call,
                    original_params,
                    cancel.clone(),
                    iteration,
                )
                .await?;
            if r.is_some() {
                last_result = r;
            }
        }
        Ok(last_result)
    }

    pub fn emit(&self, event: Event) {
        Self::emit_static(&self.events.event_tx, event);
    }

    /// Static helper for emitting without holding an `AppState` reference.
    pub fn emit_static(
        tx: &broadcast::Sender<Event>,
        event: Event,
    ) {
        if let Err(e) = tx.send(event) {
            warn!(error = %e, "event broadcast failed (no subscribers)");
        }
    }
}


#[cfg(test)]
mod tests;