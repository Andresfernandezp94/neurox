> **HISTORICAL**: este documento menciona `adan`/`eva` (nombres
> pre-rename). El agente canónico del workspace es `default`.
> Ver [EP-0009](../../changes/EP-0009-purge-non-default-agent-names/)
> para el rename completo. Para git history exacto:
> `git log --all --grep="adan|eva"`.

# ADR-0001 — Multi-agent subprocess architecture (one subprocess per session per agent)

> **Status**: accepted
> **Date**: 2026-08-15
> **Deciders**: @Andres
> **Related**: EP-0004

## Context

El daemon `neurox` ejecuta la lógica de un agent IA hablando con un LLM.
Históricamente, el "agent" era un módulo in-process del daemon (un
`tokio::spawn` con estado en memoria). Cuando se introdujo soporte multi-agent
(varios agents corriendo en paralelo, cada uno con su propio contexto, sus
tools y su identidad), se necesitaba decidir **cómo** ejecutar cada agent:

1. **Opción A**: un solo proceso daemon que mantiene N "agents" in-process,
   cada uno con su propio tokio task, identidad y working memory en memoria.
   Comparten el event loop y el LLM client.

2. **Opción B**: un subprocess separado por agent (JSON-RPC sobre stdio).
   Cada subprocess es un binario `agent` que carga su propia identidad
   desde un `--identity-dir` flag, tiene su propio LLM client, su propio
   working memory, y muere cuando el agent termina.

3. **Opción C**: agents separados por sesión, no por agent_id. Un agent
   pool con un subprocess por (session_id, agent_id), que se mata al
   cerrar la sesión.

## Decision

**Opción C con bases de Opción B** — un subprocess por (session_id,
agent_id). Cada sesión tiene su propio subprocess agent, atado a un
agent_id específico. Cuando el user cambia de agente mid-sesión (live
switch), el daemon mata el subprocess viejo y arranca uno nuevo para
el nuevo agent_id, inyectando el historial de la sesión vía el método
JSON-RPC `seed_history`.

## Consequences

**Positivas**:
- **Aislamiento real**: cada sesión tiene su propio working memory. No
  hay forma de que el contexto de una sesión se filtre a otra.
- **Paralelismo verdadero**: N sesiones concurrentes = N subprocesses
  corriendo LLM en paralelo, sin GIL ni scheduler compartido.
- **Identidad estricta**: cada agent carga system prompt + facts +
  skills desde su propio `--identity-dir` (filesystem separado). El
  daemon nunca mezcla identidades.
- **Tools por agent**: `tools_allowlist` se inyecta como env var
  (`NEUROX_TOOLS_ALLOWLIST`) al spawn. Eva solo ve 10 tools read-only.
  Adan ve todas.
- **Restart granular**: matar una sesión (idle eviction, cancel) es solo
  `kill -9` del PID correspondiente. No afecta otras sesiones.

**Negativas**:
- **Latencia del primer mensaje**: spawn del subprocess ~50-100ms. Para
  sesiones con muchos turnos, el costo se amortiza (el subprocess vive
  toda la sesión).
- **Memoria RAM**: N subprocesses × ~10-20MB cada uno. Para 50 sesiones
  concurrentes son ~500MB-1GB. Aceptable para workstation local, no
  para SaaS multi-tenant.
- **Estado efímero al switch**: cuando el user cambia de agente mid-sesión,
  el subprocess viejo se mata. El working memory del viejo se pierde
  (excepto por lo que ya se persistió en la DB). El nuevo arranca con
  historial inyectado pero sin in-context memory del anterior.

**Neutras**:
- El binario es uno solo (`agent`); la separación entre agent y default es
  puramente por `--id` + `--identity-dir` flags. Esto mantiene una sola
  codebase para mantener, pero significa que el binario carga CUALQUIER
  identidad — un bug en el binario afecta a todos los agents.

## Implementation

- `daemon/agents/agent/src/main.rs` — binario único, JSON-RPC sobre stdio.
- `daemon/core/src/session_agents.rs` — `SessionAgentPool` mantiene un map
  `session_id → Arc<SessionAgent>` con el subprocess vivo.
- `daemon/core/src/router/mod.rs` — `dispatch_to_agent` chequea si el
  `agent_id` del body matchea el subprocess vivo; si no, mata y arranca
  uno nuevo.
- `daemon/agents/agent/src/main.rs` — método `seed_history` (JSON-RPC)
  para bulk-load del working memory en el switch.

## Alternatives considered (más detalle)

- **Opción A (in-process)**: rechazada por el problema del working
  memory compartido. Cada tokio task tendría que tener su propio
  estado aislado, y el código se vuelve propenso a data races. Además,
  el LLM streaming se serializa por el event loop compartido, perdiendo
  el paralelismo real.
- **Opción C sin seed_history**: rechazada porque el nuevo agent no
  tendría contexto de la conversación previa. El user tendría que
  empezar de cero cada vez que cambia de agente, lo que rompe el caso
  de uso principal (handoff de contexto entre agents).

## References

- `daemon/agents/agent/src/main.rs` — implementación del binario.
- `daemon/core/src/session_agents.rs` — pool de subprocesses.
- `.sdd/changes/EP-0004-multi-agent-live-switch/proposal.md` — épica
  completa con los 8 fixes.
