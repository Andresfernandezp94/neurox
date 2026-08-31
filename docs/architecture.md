# Arquitectura de neurox

Este documento describe la arquitectura del producto. Para decisiones
inmutables (ADRs), vocabulario canónico y metodología, ver [`../.sdd/`](../.sdd).

## 1. Visión general

neurox sigue un modelo **núcleo + thin clients**:

- Un **daemon** en Rust es la única fuente de estado y lógica.
- Los **clientes** (web React, sidebar QML) sólo renderizan y consumen la API.

El daemon corre como `systemd user service` y escucha en
`127.0.0.1:7878` (HTTP + SSE).

## 2. El daemon

Crate principal: [`daemon/core`](../daemon/core). Construido con **axum**
(HTTP) y **tokio** (async runtime).

Responsabilidades:

- **Sesiones**: crear, listar, renombrar, borrar, cancelar. Persistidas en
  SQLite (`~/.local/share/neurox/neurox.db`).
- **Streaming de chat**: `POST /v1/sessions/:id/messages/stream` (SSE).
- **Proveedores LLM**: registro y estado (MiniMax, Anthropic, Mistral,
  OpenRouter, OpenAI, etc.). CRUD + descubrimiento de modelos por proveedor.
- **Pool de agentes** (`session_agents.rs`): un subproceso `agent` por sesión.
- **Tools engine** (crate [`daemon/tools-engine`](../daemon/tools-engine)):
  herramientas nativas + backends de LLM.
- **Gestión de env/keys**: `PUT/DELETE /v1/env/:key`, con un watcher que
  propaga cambios de `~/.config/neurox/env` al proceso.

### Estructura de crates

```
daemon/
├── core/            ← binario `neurox`: router HTTP, sesiones, pool de agentes
├── tools-engine/    ← tools nativas + backends LLM (minimax, anthropic, openai_compat)
├── crates/agent-lib/← librería compartida por el binario `agent` (LLM client, memoria)
└── agents/agent/    ← binario `agent`: el subproceso que corre por sesión
```

## 3. Modelo de sesiones y agentes

**Decisión central**: el agente `default` es un **session agent**, no un
proceso compartido. Cada `POST /v1/sessions` con `agent_id: "default"`
**spawnea un subproceso `agent` dedicado** (`SessionAgentPool::start_for_session`).

```
create_session(agent_id) ──► is_session_agent(agent_id)?
                                   │ sí
                                   ▼
                         start_for_session(session_id, agent_id)
                                   │  (spawn subproceso `agent`)
                                   ▼
                         SessionAgentPool.agents[session_id] = <subproceso>
```

Beneficios:

- **Paralelismo real**: dos sesiones (p.ej. una en la web y otra en el
  sidebar) corren en subprocesos distintos, con PIDs distintos, en paralelo.
- **Aislamiento**: cada subproceso tiene su propia *working memory*.
- **Resiliencia**: si un subproceso muere o se cuelga, sólo afecta a su sesión.
- **Idle eviction**: una sesión inactiva N segundos (`idle_timeout_secs`,
  default 1800) mata su subproceso; se re-spawnea en el siguiente mensaje y
  su historial se re-siembra desde la DB (`seed_history`).

> Históricamente `default` era un único subproceso *persistente* compartido.
> Cuando moría, todos los clientes quedaban sin respuesta y no había
> paralelismo. La migración a *session agent* resolvió esto.

## 4. Flujo de un mensaje (streaming)

```
Cliente                         Daemon                         Subproceso agent
  │  POST /v1/sessions            │                                  │
  │ ────────────────────────────►│  start_for_session ─────────────►│ (spawn)
  │  {session_id, pid}            │                                  │
  │ ◄────────────────────────────│                                  │
  │                               │                                  │
  │  POST .../messages/stream     │                                  │
  │  {agent_id, text}             │  dispatch_to_agent               │
  │ ────────────────────────────►│  (seed_history si 1ª vez) ──────►│
  │                               │  stream(process) ───────────────►│
  │  SSE: {type:thinking,...}     │ ◄─── content_delta/thinking_delta │
  │ ◄────────────────────────────│  (event bus → SSE)               │
  │  SSE: {type:content,...}      │ ◄──────────────────────────────  │
  │ ◄────────────────────────────│                                  │
  │  (fin de conexión / [DONE])   │ ◄─── respuesta JSON-RPC final ── │
  │ ◄────────────────────────────│  Event::Done                     │
```

### Contrato del stream

- **Request**: `POST /v1/sessions/:id/messages/stream` con body
  `{"agent_id": "<id>", "text": "<mensaje>"}`.
- **Response** (SSE): líneas `data: {...}` con `type` ∈
  `thinking | content | tool_call | tool_result | error`, y cierre por fin de
  conexión (más `Event::Done` cuando el dispatch retorna).

El sidebar (`NeuroxApiStrategy`) y la web (`streamMessage`) usan **el mismo
contrato**.

## 5. Protocolo daemon ↔ agent

JSON-RPC sobre stdio. El daemon escribe una request `process` al stdin del
subproceso; el subproceso emite:

- **notificaciones** (sin `id`): `content_delta`, `thinking_delta`,
  `tool_call` → el daemon las reenvía al event bus como eventos SSE.
- **respuesta final** (con `id`): cierra el turno. El daemon entonces emite
  `Event::Done` y persiste el mensaje del asistente.

> El agente debe cerrar su turno enviando la respuesta final incluso cuando
> el backend LLM no emite un evento explícito de "done": el fin del stream de
> tokens debe traducirse en el retorno de `chat_stream`.

## 6. Backends de LLM

En `daemon/tools-engine/src/backend/`:

- `minimax.rs` — SSE OpenAI-compatible; separa `<think>` (thinking) del content.
- `anthropic.rs` — eventos `content_block_*`; base URL sin `/v1` duplicado.
- `openai_compat.rs` — genérico para proveedores OpenAI-compatibles.

El descubrimiento de modelos es **por proveedor**
(`GET /v1/llm/providers/:id/models`) y devuelve un array de strings con los
model ids. Los modelos **no se hardcodean**: se obtienen del proveedor.

## 7. Clientes

### Cliente web (`client/web`)

React + Vite + TypeScript. Piezas clave:

- `api/sessions.ts` — `createSession`, `sendMessage`, `streamMessage` (SSE).
- `api/llm.ts` — proveedores y modelos; `listModels` normaliza el array de
  strings del daemon a `{id}`.
- `components/ModelSelector.tsx` — dropdown de modelos agrupados por proveedor.
- `components/ProvidersPanel.tsx` — gestión de proveedores y API keys.

### Sidebar (Quickshell / QML)

Fuera de este repo (config de escritorio). Es la **referencia visual**; la web
se alinea a su look & feel. Habla el mismo protocolo vía `NeuroxApiStrategy`.

## 8. Seguridad

- Las API keys viven **fuera del repo**, en `~/.config/neurox/env` (`0600`),
  cargadas por el `EnvironmentFile` del servicio systemd y por un watcher.
- El daemon escucha sólo en `127.0.0.1` (loopback).
- El `.gitignore` raíz excluye `target/`, `node_modules/`, `*.env`, `*.key`,
  `*.pem`, `*credentials*`, `*.db`, backups y logs.

## 9. Operación

```bash
systemctl --user restart neurox.service   # reiniciar
systemctl --user status  neurox.service   # estado
journalctl --user -u neurox.service -f     # logs en vivo
```

Al reinstalar el binario `agent`, detené el daemon y matá subprocesos `agent`
vivos primero (el binario queda *busy* si hay procesos usándolo):

```bash
systemctl --user stop neurox.service
pkill -9 -x agent
cp daemon/target/release/agent ~/.local/bin/agent
systemctl --user start neurox.service
```
