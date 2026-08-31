# neurox

> Un daemon local de orquestación de agentes de IA, con clientes ligeros
> (sidebar de escritorio y web) que comparten un único núcleo.

**neurox** es un orquestador de agentes de IA que corre como servicio local
(`systemd user service`). El **daemon** es el núcleo único: administra
proveedores de LLM, sesiones de chat, agentes, herramientas (tools) y el
streaming de respuestas. Los clientes —un **sidebar** de escritorio
(Quickshell/QML) y un **cliente web** (React)— son *thin clients* que hablan
el mismo protocolo HTTP/SSE contra el daemon y comparten los mismos datos:
sesiones, proveedores, API keys, modelos y agentes.

```
        ┌──────────────────┐        ┌──────────────────┐
        │  Sidebar (QML)   │        │   Web (React)    │
        │  thin client     │        │   thin client    │
        └────────┬─────────┘        └─────────┬────────┘
                 │  HTTP + SSE                 │  HTTP + SSE
                 │  (127.0.0.1:7878)           │
                 └──────────────┬──────────────┘
                                ▼
                    ┌───────────────────────┐
                    │   neurox daemon (Rust) │
                    │  ─────────────────────  │
                    │  • sesiones + streaming │
                    │  • proveedores LLM      │
                    │  • pool de agentes      │
                    │  • tools engine         │
                    └───────────┬────────────┘
                                │  JSON-RPC / stdio (un subproceso por sesión)
                                ▼
                    ┌───────────────────────┐
                    │  agent (subproceso)   │  ← uno por sesión, aislado
                    │  identity + memory +   │
                    │  skills + LLM backend  │
                    └───────────────────────┘
```

## Principio de diseño

- **El daemon es el núcleo único.** Toda la lógica de estado (sesiones,
  proveedores, agentes, keys, modelos) vive en el daemon.
- **Los clientes son ligeros.** Sidebar y web no duplican lógica: consumen
  la misma API y comparten datos. Una sesión creada en la web y otra en el
  sidebar coexisten de forma independiente.
- **Aislamiento por sesión.** Cada sesión de chat obtiene su **propio
  subproceso `agent`** con memoria de trabajo aislada. Esto da paralelismo
  real: una sesión ocupada o caída nunca bloquea a otra.

## Componentes

| Componente | Ruta | Stack | Rol |
|-----------|------|-------|-----|
| **daemon** | [`daemon/`](./daemon) | Rust (axum, tokio) | Núcleo: API HTTP/SSE, sesiones, proveedores LLM, pool de agentes, tools engine |
| **agent** | [`daemon/agents/agent/`](./daemon/agents) | Rust | Binario del agente: identidad, memoria, skills, backends de LLM (MiniMax, Anthropic, OpenAI-compat) |
| **client/web** | [`client/web/`](./client/web) | React + Vite + TypeScript | Cliente web: chat, selector de modelos/agentes, panel de proveedores |
| **sidebar** | *(config Quickshell, fuera de este repo)* | QML | Cliente de escritorio (referencia visual) |
| **agents** | [`agents/`](./agents) | JSON + Markdown | Plantillas versionadas de agentes (identidad, prompt, skills) |
| **.sdd** | [`.sdd/`](./.sdd) | Markdown | Source of Truth: gobernanza, ADRs, glosario, metodología |

## Quickstart

Requisitos: Rust (stable), Node 18+, y las API keys de tus proveedores LLM.

### 1. Compilar e instalar el daemon y el agente

```bash
cd daemon
cargo build --release
install -Dm755 target/release/neurox ~/.local/bin/neurox
install -Dm755 target/release/agent  ~/.local/bin/agent
```

### 2. Configurar las API keys (FUERA del repo)

Las keys **nunca** viven en el repositorio. Se cargan desde un archivo de
entorno privado (`~/.config/neurox/env`, permisos `0600`):

```bash
mkdir -p ~/.config/neurox
cat > ~/.config/neurox/env <<'EOF'
MINIMAX_API_KEY=...
ANTHROPIC_API_KEY=...
MISTRAL_API_KEY=...
OPENROUTER_API_KEY=...
EOF
chmod 600 ~/.config/neurox/env
```

Ver [`daemon/config.example.yaml`](./daemon/config.example.yaml) para la
configuración del daemon (proveedores, agentes de sesión).

### 3. Arrancar el daemon (systemd user service)

```bash
systemctl --user start neurox.service
curl -s http://127.0.0.1:7878/health   # → {"status":"ok", ...}
```

### 4. Levantar el cliente web

```bash
cd client/web
npm install
npm run dev        # http://localhost:5173
```

## API (resumen)

El daemon expone una API HTTP en `127.0.0.1:7878`. Endpoints clave:

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET`  | `/health` | Estado del daemon |
| `GET/POST` | `/v1/sessions` | Listar / crear sesiones (spawnea un subproceso por sesión) |
| `POST` | `/v1/sessions/:id/messages/stream` | Chat en streaming (SSE). Body: `{"agent_id","text"}` |
| `GET`  | `/v1/agents` | Agentes disponibles |
| `GET`  | `/v1/llm/providers` | Proveedores LLM y su estado |
| `GET`  | `/v1/llm/providers/:id/models` | Modelos de un proveedor |
| `PUT/DELETE` | `/v1/env/:key` | Gestionar API keys en runtime |

Contrato de eventos SSE: `data: {"type":"thinking"|"content"|"tool_call"|"tool_result"|"error", ...}`.

## Documentación

- **Arquitectura**: [`docs/architecture.md`](./docs/architecture.md)
- **Gobernanza / metodología / ADRs**: [`.sdd/`](./.sdd) (Source of Truth)
- **Glosario canónico**: [`.sdd/GLOSSARY.md`](./.sdd/GLOSSARY.md)
- **Contribuir**: [`CONTRIBUTING.md`](./CONTRIBUTING.md)

## Licencia

Ver [`LICENSE`](./LICENSE).
