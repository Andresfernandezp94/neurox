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

/// Fase 2 del despacho por lotes: ejecutar las tools EN PARALELO.
///
/// EP-2026-10-03: params del turno de soft cap.
///
/// Vive en una fn aparte, y SIN envoltorio `"params"`, por dos razones que
/// son el mismo error:
///
/// - El transporte (`protocols::json_rpc_stdio`) ya envuelve lo que le
///   pases en `AgentRequest::params` dentro de una clave `"params"`. Si aqui
///   se vuelve a envolver, el mensaje llega al agente como
///   `params.params.params.text` y `handle_process` lee un texto VACIO.
///   Pasaba de verdad: el "STOP making tool calls" no llegaba nunca al
///   modelo, que recibia un turno de usuario en blanco con `tools: []`. El
///   proveedor devolvia 400 `invalid_request_error` y el turno moria con
///   "soft-cap dispatch failed".
/// - Al ser una fn pura, la forma se puede testear sin montar un agente.
///
/// `tools: []` es deliberado: deja al modelo sin forma estructural de
/// seguir emitiendo tool_calls.
fn iteration_limit_params(iteration: u32) -> serde_json::Value {
    let text = format!(
        "[system] You've iterated {iteration} times on this request. \
         STOP making tool calls. Summarize what you've found so far \
         and ask the user for next steps, OR take a definitive action. \
         Your next response MUST be a final assistant message with no \
         tool_calls — otherwise this loop will hard-stop with \
         IterationLimit."
    );
    serde_json::json!({ "text": text, "tools": [] })
}

/// EP-2026-10-03. Es una fn libre y no un metodo a proposito: el paralelismo
/// es la unica parte del despacho que se puede testear sin montar un agente
/// subprocess, asi que vive aislada para que el test la pueda cronometrar.
///
/// Que solape de verdad depende de que las tools cedan el control: si una
/// llama a I/O bloqueante (`std::fs`, `std::process::Command`) se traga el
/// hilo y el lote se vuelve secuencial por el camino deEscape. Hoy todas
/// usan `tokio::fs` / `tokio::process` / `reqwest`, que ceden.
///
/// `join_all` (no `try_join_all`) a proposito: el fallo de una tool no debe
/// cancelar a las demas. El LLM necesita ver TODOS los resultados del lote,
/// incluido el del fichero que no existe, para decidir el siguiente paso.
///
/// Devuelve un item por tool, en el MISMO orden de `pending` (el orden de
/// completacion es libre, el de entrega no). `None` = esa entrada ya se
/// cerro sola en la fase 1 y no hay nada que ejecutar.
async fn exec_batch_parallel(
    ctx: &tools_engine::ExecuteContext,
    pending: &[PreparedToolCall],
) -> (Vec<Option<Result<String, String>>>, std::time::Duration) {
    let start = std::time::Instant::now();
    let joined = futures::future::join_all(pending.iter().map(|slot| {
        let ctx = &ctx;
        async move {
            match &slot.tool {
                Some(tool) => Some(tool.execute(ctx, slot.call.args.clone()).await),
                // Ya resuelta en la fase 1 (denegada o inexistente): no
                // hay nada que ejecutar, pero conserva su hueco en la
                // salida para no desalinear los indices.
                None => None,
            }
        }
    }))
    .await;
    (joined, start.elapsed())
}

/// Una tool_call resuelta y lista para ejecutar.
///
/// EP-2026-10-03: sale de `prepare_tool_call` y es lo que permite separar
/// "resolver + aprobar" (secuencial, interactivo con el usuario) de
/// "ejecutar" (paralelo). `Arc<dyn Tool>` porque el registro devuelve esa
/// referencia y hay que poder clonarla a cada task del `join_all`.
struct PreparedToolCall {
    call: ToolCall,
    /// `None` si la tool existe y hay que ejecutarla. `Some(texto)` si ya
    /// tiene resultado sin ejecutar: tool denegada por el usuario o tool
    /// que el modelo se invento y no esta registrada.
    ///
    /// EP-2026-10-03: estos casos NO se despachan al agente al vuelo. Se
    /// guardan y salen en la misma tanda que el resto, porque partir el
    /// lote dejaria un `assistant` de N tool_calls con solo algunos
    /// resultados, que es lo que el proveedor rechaza con 400.
    tool: Option<Arc<dyn tools_engine::Tool>>,
    output: String,
}

impl AppState {
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
    /// EP-0003 Tier 2: soft cap en `MAX_TOOL_ITERATIONS`. En vez de
    /// fallar en seco, el daemon le inyecta al agente un mensaje sintetico
    /// pidiéndole que resuma y cierre el turno, y le da UNA iteracion mas
    /// para responder con texto final. Si el LLM insiste con tool_calls,
    /// `AgentError::IterationLimit` corta el bucle.
    ///
    /// EP-2026-10-03: extraido de `handle_session_tool_call` para que el
    /// camino de lote lo herede sin duplicar el texto sintetico (que es
    /// parte del prompt, no un detalle de UI: si las dos copias divergen,
    /// el modelo recibe instrucciones distintas segun cuantos tools habia).
    ///
    /// Los eventos salen por el forwarder de siempre, asi que el usuario ve
    /// el resumen en streaming. Devuelve siempre `Ok(None)`: no hay tool
    /// que ejecutar ni resultado que devolver.
    async fn inject_iteration_limit(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let synthetic_params = iteration_limit_params(iteration);

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
        Ok(None)
    }

    /// Fase 1 del despacho: parsear la tool_call, resolver la tool,
    /// emitir `Event::ToolCall` y pedir aprobacion si corresponde.
    ///
    /// EP-2026-10-03: extraido de `handle_session_tool_call` para que el
    /// camino de lote y el de una sola tool NO dupliquen la logica de
    /// aprobacion. Nunca se ejecuta la tool aca: eso es la fase 2.
    ///
    /// `Ok(prepared)` = hay que ejecutarla (fase 2).
    /// `Err(follow_up)` = el call ya se cerro solo —tool desconocida, o el
    /// usuario la denego— y su `ToolResult` ya fue emitido Y despachado al
    /// agente. Quien llama no ejecuta nada, pero debe propagar
    /// `follow_up` como ultima respuesta del turno si viene alguno: al
    /// Al despachar "[denied by user]" el agente puede responder con texto
    /// final o con mas tool_calls, y descartar eso deja el turno a medias.
    async fn prepare_tool_call(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        tool_call_value: serde_json::Value,
        iteration: u32,
    ) -> anyhow::Result<Option<PreparedToolCall>> {
        let call: ToolCall = serde_json::from_value(tool_call_value.clone())
            .map_err(|e| anyhow::anyhow!("invalid tool_call: {e}"))?;
        let call_name = call.name.clone();

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
            // Sin `Event::ToolCall`: no hay tool que ejecutar, asi que no
            // hay nada que mostrar girando. El error sale en el
            // `ToolResult` de la fase 3, que emite toda la tanda.
            return Ok(Some(PreparedToolCall {
                call,
                tool: None,
                output: format!("[error] unknown tool: {}", call_name),
            }));
        };

        let spec = tool.spec();

        let _ = self.events.event_tx.send(Event::ToolCall {
            session_id,
            tool: call.name.clone(),
            args: call.args.clone(),
            iteration,
            call_id: call.id.clone(),
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
                // Se registra la decision y se sigue: el resultado de
                // "denegada" viaja en la tanda de la fase 3.
                return Ok(Some(PreparedToolCall {
                    call,
                    tool: None,
                    output: "[denied by user]".to_string(),
                }));
            }
        }
        Ok(Some(PreparedToolCall {
            call,
            tool: Some(tool),
            output: String::new(),
        }))
    }

    /// Fase 3 (de una tool): registrar la ejecucion y emitir el
    /// `Event::ToolResult`. Devuelve el texto que se le pasa al agente.
    ///
    /// EP-2026-10-03: existe para que el camino de una tool y el de un lote
    /// reporten los errores IGUAL. Antes solo el camino de una tool mandaba
    /// `Event::Error` ademas del `ToolResult`, asi que el mismo fallo se
    /// mostraba como banner rojo global con una tool y no con dos.
    ///
    /// El error va SOLO dentro del `ToolResult`, no como `Event::Error`, por
    /// dos motivos:
    ///   - El LLM necesita leerlo como resultado para decidir el siguiente
    ///     paso; un error de tool es parte del turno, no un fallo del daemon.
    ///   - El cliente ya lo muestra en su sitio: la entrada del tool en el
    ///     timeline. El banner global de error reserva para lo que si es un
    ///     fallo del daemon (dispatch caido, bus cerrado).
    ///
    /// OJO: esta fn EMITE el `ToolResult`. Quien la llame no debe mandarlo
    /// otra vez. Pasaba en el despacho por lotes, y el cliente recibia dos
    /// eventos con el mismo `call_id` y el mismo texto, que se persistian
    /// como dos filas y se pintaban como dos tools.
    ///
    /// Nota: `Event::Error` ya no cierra el stream — ver el fix D2 en
    /// router/http.rs (~linea 1664). Cuando lo cerraba, mandarlo desde el
    /// camino de una tool se tragaba el `ToolResult` de la propia tool.
    fn emit_tool_result(
        &self,
        agent_id: &str,
        session_id: Uuid,
        iteration: u32,
        call: &ToolCall,
        result: Result<String, String>,
        duration: std::time::Duration,
        batch: Option<(usize, std::time::Duration)>,
    ) -> String {
        let (output, failed) = match result {
            Ok(s) => (s, false),
            Err(e) => {
                tracing::warn!(
                    agent_id,
                    session_id = %session_id,
                    tool = %call.name,
                    error = %e,
                    "tool failed"
                );
                (format!("[error] {e}"), true)
            }
        };
        tracing::info!(
            agent_id,
            session_id = %session_id,
            tool = %call.name,
            outcome = if failed { "err" } else { "ok" },
            duration_ms = duration.as_millis() as u64,
            batch_size = batch.map(|(n, _)| n),
            batch_total_ms = batch.map(|(_, t)| t.as_millis() as u64),
            "{}",
            if batch.is_some() { "tool_executed (parallel batch)" } else { "tool_executed" }
        );
        let _ = self.events.event_tx.send(Event::ToolResult {
            session_id,
            tool: call.name.clone(),
            result: output.clone(),
            iteration,
            call_id: call.id.clone(),
            seq: self.events.next_seq(session_id),
        });
        output
    }

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
    /// Camino de UNA sola tool_call: fase 1, ejecucion, y despacho.
    ///
    /// EP-2026-10-03: el trabajo real esta en `prepare_tool_call` (fase 1)
    /// y en `dispatch_session_tool_result` (fase 3). Esta fn las cose con
    /// una unica ejecucion en medio. Para un lote usa
    /// `handle_session_tool_calls_batch`, que comparte la fase 1.
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
            return self.inject_iteration_limit(agent, session_id, iteration).await;
        }
        let prepared = match self
            .prepare_tool_call(agent.clone(), session_id, tool_call_value, iteration)
            .await?
        {
            Some(p) => p,
            // `tool_call` mal formado: no hay nada que despachar.
            None => return Ok(None),
        };
        let call = prepared.call;
        let Some(tool) = prepared.tool else {
            // Tool desconocida o denegada: ya tiene su texto. Se emite su
            // ToolResult y se devuelve al agente como cualquier otra.
            let denied = prepared.output;
            let _ = self.events.event_tx.send(Event::ToolResult {
                session_id,
                tool: call.name.clone(),
                result: denied.clone(),
                iteration,
                call_id: call.id.clone(),
                seq: self.events.next_seq(session_id),
            });
            return self
                .dispatch_session_tool_result(
                    agent,
                    session_id,
                    &call,
                    &denied,
                    original_params,
                    cancel,
                    iteration,
                )
                .await;
        };

        let ctx = tools_engine::ExecuteContext {
            agent_id: agent.agent_id.clone(),
            cancel: None,
            http_client: None,
        };
        let exec_start = std::time::Instant::now();
        let result = tool.execute(&ctx, call.args.clone()).await;
        let exec_duration = exec_start.elapsed();

        let output = self.emit_tool_result(
            &agent.agent_id,
            session_id,
            iteration,
            &call,
            result,
            exec_duration,
            None,
        );

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

    /// EP-2026-10-03: devuelve al agente TODOS los resultados de un lote
    /// con UNA sola llamada (`tool_results`), en vez de una por tool.
    ///
    /// El motivo esta en `handle_tool_results` del agente: el historial
    /// tiene que quedar bien formado (un mensaje `tool` por cada
    /// `tool_call` del mensaje `assistant`), y una llamada al LLM por
    /// resultado lo deja a medias cuando el lote tiene mas de una tool.
    ///
    /// A partir de la respuesta, la recursion sobre las siguientes
    /// tool_calls es la misma de `dispatch_session_tool_result`.
    async fn dispatch_session_tool_results(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        batch: Vec<(ToolCall, String)>,
        original_params: &serde_json::Value,
        cancel: CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if batch.is_empty() {
            return Ok(None);
        }
        let results: Vec<serde_json::Value> = batch
            .iter()
            .map(|(call, output)| {
                serde_json::json!({ "call_id": call.id, "result": output })
            })
            .collect();
        let params = serde_json::json!({ "results": results });

        let resp = self
            .stream_session_agent(
                agent.clone(),
                "tool_results",
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
                    .unwrap_or_else(|| "agent returned error after tool results".into()),
            });
            return Ok(None);
        }

        // Mismo tratamiento que el camino de una sola tool: si el modelo
        // responde con mas tool_calls, se despachan como lote nuevo.
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

/// D1 fix: dispatch de un lote de tool_calls emitido por el modelo en un
    /// mismo turno.
    ///
    /// EP-2026-10-03: las tools se EJECUTAN en paralelo y sus resultados se
    /// devuelven al agente DE UNA VEZ. Las dos mitades son necesarias y
    /// ninguna basta sola:
    ///
    /// - Ejecutar en paralelo: `tool.execute()` es I/O (shell, web_fetch,
    ///   read_file) y no toca el estado del agente. Tres ficheros o tres
    ///   peticiones tardan el max, no la suma.
    /// - Devolver los resultados juntos: el agente hace UNA llamada al LLM
    ///   por cada tanda de resultados. Mandarlos de uno en uno deja el
    ///   historial con un `assistant` de 3 `tool_calls` seguido de un solo
    ///   mensaje `tool`, y el proveedor responde 400 `invalid_request_error`
    ///   porque un `assistant` con tool_calls debe ir seguido de un
    ///   mensaje `tool` por cada `tool_call_id`. Se vio en produccion: el
    ///   agente se quejaba de "read_file only returned results for one".
    ///
    /// La aprobacion (fase 1) sigue secuencial a proposito: pedir N
    /// permisos simultaneos en la UI es ilegible.
    ///
    /// Orden de eventos: todos los `ToolCall` primero (en orden del modelo),
    /// despues todos los `ToolResult` (tambien en orden). Es lo que un
    /// humano espera de un "run these in parallel". El orden de COMPLETION
    /// es libre, asi que los resultados se guardan por indice y se emiten
    /// en el loop final, no al terminar cada task.
    async fn handle_session_tool_calls_batch(
        &self,
        agent: Arc<crate::session_agents::SessionAgent>,
        session_id: Uuid,
        calls: Vec<serde_json::Value>,
        original_params: &serde_json::Value,
        cancel: CancellationToken,
        iteration: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if calls.is_empty() {
            return Ok(None);
        }
        // El soft cap se comprueba aqui y no solo en el camino de una sola
        // tool: un lote de 6 tools en la iteracion N no puede saltarselo.
        // Se comprueba una vez, no por tool: la decision de parar es del
        // turno, no de cada llamada.
        if iteration >= max_tool_iterations() {
            return self.inject_iteration_limit(agent, session_id, iteration).await;
        }
        // Con una sola tool_call no se gana nada con el lote: se sigue el
        // camino simple y se evita el overhead del join.
        if calls.len() == 1 {
            let only = calls.into_iter().next().expect("len 1");
            return self
                .handle_session_tool_call(
                    agent,
                    session_id,
                    only,
                    original_params,
                    cancel,
                    iteration,
                )
                .await;
        }

        // ── fase 1: resolver + aprobar (secuencial, en orden) ───────────
        //
        // NO se despacha nada al agente aca, ni siquiera para una tool
        // denegada o desconocida: esas tambien se acumulan para la fase 3,
        // que manda el lote entero de golpe. Despachar aqui partia el turno
        // en dos y era justo lo que el proveedor rechazaba.
        let mut prepared: Vec<PreparedToolCall> = Vec::with_capacity(calls.len());
        for value in calls {
            match self
                .prepare_tool_call(agent.clone(), session_id, value, iteration)
                .await?
            {
                // `Resolved` = tool desconocida o denegada por el usuario:
                // ya tiene su texto de resultado y solo hay que emitirlo y
                // incluirlo en la tanda.
                Some(p) => prepared.push(p),
                None => continue,
            }
        }
        if prepared.is_empty() {
            // Todas las calls se resolvieron solas y ninguna llego a
            // ejecutarse: no hay nada que devolver. (Con la fase 3 unificada
            // esto solo se da si el lote venia vacio de verdad.)
            return Ok(None);
        }

        // ── fase 2: ejecutar en paralelo ────────────────────────────────
        let ctx = tools_engine::ExecuteContext {
            agent_id: agent.agent_id.clone(),
            cancel: None,
            http_client: None,
        };
        let (joined, batch_elapsed) = exec_batch_parallel(&ctx, &prepared).await;
        // Reparto uniforme: sin task por tool no hay cronometro individual.
        // Se registra el total del lote, que es el numero que importa para
        // decidir si el paralelismo sirve.
        let per_tool = batch_elapsed / prepared.len() as u32;

        // ── fase 3: emitir resultados en orden y despachar LA TANDA ────
        let mut batch: Vec<(ToolCall, String)> = Vec::with_capacity(prepared.len());
        for (slot, outcome) in prepared.iter().zip(joined) {
            let output = match outcome {
                // Recién ejecutada. `emit_tool_result` cronometra, loguea
                // Y EMITE el evento: no hay que mandarlo otra vez aqui.
                Some(result) => self.emit_tool_result(
                    &agent.agent_id,
                    session_id,
                    iteration,
                    &slot.call,
                    result,
                    per_tool,
                    Some((prepared.len(), batch_elapsed)),
                ),
                // Ya resuelta en la fase 1 (denegada por el usuario o tool
                // inexistente). No hay ejecucion que cronometrar ni loguear
                // como tool ejecutada, asi que el evento se emite aqui.
                None => {
                    let out = slot.output.clone();
                    let _ = self.events.event_tx.send(Event::ToolResult {
                        session_id,
                        tool: slot.call.name.clone(),
                        result: out.clone(),
                        iteration,
                        call_id: slot.call.id.clone(),
                        seq: self.events.next_seq(session_id),
                    });
                    out
                }
            };
            batch.push((slot.call.clone(), output));
        }

        self.dispatch_session_tool_results(agent, session_id, batch, original_params, cancel, iteration)
            .await
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
            call_id: call.id.clone(),
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
            call_id: call.id.clone(),
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