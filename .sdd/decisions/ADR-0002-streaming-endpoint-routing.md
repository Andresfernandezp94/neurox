> **HISTORICAL**: este documento menciona `adan`/`eva` (nombres
> pre-rename). El agente canónico del workspace es `default`.
> Ver [EP-0009](../../changes/EP-0009-purge-non-default-agent-names/)
> para el rename completo. Para git history exacto:
> `git log --all --grep="adan|eva"`.

# ADR-0002 — Streaming endpoint routes through `dispatch_to_agent`, not `llmd`

> **Status**: accepted
> **Date**: 2026-08-15
> **Deciders**: @Andres
> **Related**: EP-0004

## Context

El frontend usa dos endpoints de chat:
- `POST /v1/sessions/:id/messages` (non-streaming) — para tests/scripts.
- `POST /v1/sessions/:id/messages/stream` (SSE streaming) — para el chat
  interactivo.

Históricamente, los dos endpoints funcionaban diferente:

- **Non-streaming** (`post_message`): rutaba via `dispatch_to_agent` →
  JSON-RPC al subprocess agent → agent llama MiniMax API → respuesta.
- **Streaming** (`post_message_stream`): rutaba via `llmd_client.chat_stream`
  → HTTP a `http://127.0.0.1:9997/v1/chat/stream` → llmd llamaba MiniMax →
  SSE events.

El llmd (`mcps/llmd/`) es un servicio Rust standalone que tiene:
- Su propio system prompt hardcoded ("Soy EVA, el agente persistente...").
- Su propio mapeo session_id → agent.
- Su propio store de provider/model config.

Cuando el daemon le pasaba un `ChatRequest {session_id, text}` SIN
`agent_id`, el llmd respondía siempre con "Soy EVA" porque no sabía qué
agente era — y no le importaba, porque su system prompt ya asumía que
él ERA el agente principal.

Esto causaba que **TODO el chat streaming** devolviera "Soy EVA" sin
importar qué agente seleccionara el usuario en el dropdown del frontend.
El bug se manifestaba como: el dropdown mostraba `agent ✓` pero el
agente respondía como EVA.

## Decision

El streaming endpoint usa `dispatch_to_agent(&body.agent_id, ...)` (igual
que el non-streaming). El llmd queda solo para operaciones admin (provider
CRUD, status, active provider), NO para chat.

## Consequences

**Positivas**:
- **Source of truth unificado**: el body del request lleva `agent_id` y
  el daemon lo respeta en ambos paths (streaming y non-streaming).
- **Identidad correcta**: el system prompt del agent seleccionado se
  carga desde su `--identity-dir` (agent o default), no desde el hardcoded
  del llmd.
- **Live switch funciona en streaming**: el `seed_history` ya estaba
  disponible en el subprocess agent; ahora el streaming lo usa
  naturalmente vía dispatch_to_agent.
- **Código más simple**: ~340 líneas de SSE parsing eliminadas en
  `daemon/core/src/router/http.rs`.

**Negativas**:
- **llmd queda parcialmente sin uso**: el chat no lo usa más. Sus otros
  endpoints admin siguen activos (provider CRUD, model discovery,
  HF integration, etc.). No se depreca completamente — solo se saca
  del path de chat.
- **Si en el futuro se quiere un chat "raw"** (passthrough sin agent),
  el endpoint `POST /v1/chat/raw` ya existe y sigue usando llmd.

## Implementation

- `daemon/core/src/router/http.rs` — `post_message_stream` reescrito.
  En vez de `llmd_client.chat_stream(req)`, ahora hace
  `state.dispatch_to_agent(&agent_id, "process", session_id, params)`
  y persiste el assistant message al final.
- Los eventos (Content/Thinking/ToolCall/ToolResult) ya fluyen por
  el global event bus desde `dispatch_to_session_agent`. El forward
  loop existente los captura y los manda al SSE response.
- `daemon/core/src/llmd_client.rs` — `ChatRequest` struct se mantiene
  para que `post_chat_raw` y otros endpoints admin puedan seguir
  usando el llmd.

## Alternatives considered

- **Opción A**: agregar `agent_id` al `ChatRequest` y hacer que el llmd
  respete el system prompt del agente seleccionado. Rechazada porque
  el llmd tendría que cargar los identity dirs del daemon — duplicaría
  la lógica de carga de identidad y abriría la puerta a drift entre
  el daemon y el llmd sobre qué system prompt es el vigente.
- **Opción B**: eliminar el llmd completamente. Prematura — el llmd
  tiene valor en admin (provider CRUD, HF integration, model discovery).
  El alcance de este cambio es el path de chat, no deprecar el
  servicio entero.

## References

- `daemon/core/src/router/http.rs` — `post_message_stream` (línea ~996+).
- `daemon/core/src/router/mod.rs` — `dispatch_to_agent` (línea ~622+).
- `.sdd/changes/EP-0004-multi-agent-live-switch/proposal.md` — Fix 6
  en la épica completa.
