# neurox daemon

Núcleo del producto: servicio local en Rust que orquesta sesiones, proveedores
LLM, agentes y herramientas. Expone una API HTTP/SSE en `127.0.0.1:7878`.

## Crates

| Crate | Rol |
|-------|-----|
| `core` | Binario `neurox`: router HTTP (axum), sesiones (SQLite), pool de agentes por sesión, event bus, gestión de env/keys |
| `tools-engine` | Herramientas nativas + backends de LLM (`minimax`, `anthropic`, `openai_compat`) + descubrimiento de modelos |
| `crates/agent-lib` | Librería compartida por el binario `agent`: cliente LLM, memoria, identidad |
| `agents/agent` | Binario `agent`: el subproceso que corre **por sesión** (JSON-RPC sobre stdio) |

## Build

```bash
cargo build --release
install -Dm755 target/release/neurox ~/.local/bin/neurox
install -Dm755 target/release/agent  ~/.local/bin/agent
```

## Configuración

- Config del daemon: `~/.config/neurox/config.yaml`
  (plantilla: [`config.example.yaml`](./config.example.yaml)).
- API keys: `~/.config/neurox/env` (`0600`) — **fuera del repo**.

## Ejecución

Como servicio systemd de usuario:

```bash
systemctl --user start   neurox.service
systemctl --user restart neurox.service
journalctl --user -u neurox.service -f
```

O directamente (dev):

```bash
~/.local/bin/neurox serve
```

## API (endpoints principales)

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET`  | `/health` | Estado |
| `GET/POST` | `/v1/sessions` | Listar / crear sesiones (spawnea subproceso por sesión) |
| `GET/DELETE` | `/v1/sessions/:id` | Obtener / borrar sesión |
| `POST` | `/v1/sessions/:id/messages/stream` | Chat SSE. Body `{"agent_id","text"}` |
| `POST` | `/v1/sessions/:id/cancel` | Cancelar turno en curso |
| `GET`  | `/v1/sessions/agents` | Subprocesos de sesión vivos (PIDs) |
| `GET`  | `/v1/agents` | Agentes disponibles |
| `GET/POST` | `/v1/llm/providers` | Proveedores LLM |
| `GET`  | `/v1/llm/providers/:id/models` | Modelos de un proveedor (array de strings) |
| `PUT/DELETE` | `/v1/env/:key` | Gestionar API keys en runtime |

## Modelo de sesiones

Cada sesión con `agent_id: "default"` obtiene su propio subproceso `agent`
(ver [`../docs/architecture.md`](../docs/architecture.md)). Paralelismo real,
memoria aislada, idle-eviction a 30 min.

## Tests / lint

```bash
cargo test
cargo clippy      # clippy.toml
cargo fmt         # rustfmt.toml
```

## Reinstalar el binario `agent`

El binario queda *busy* si hay subprocesos vivos. Procedimiento seguro:

```bash
systemctl --user stop neurox.service
pkill -9 -x agent
cp target/release/agent ~/.local/bin/agent
systemctl --user start neurox.service
```
