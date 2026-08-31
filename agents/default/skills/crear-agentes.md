# Skill: Crear agentes en Neurox

Esta skill explica cómo crear y mantener agentes en Neurox.
El sistema usa un manifest v2 (schema_version 2) con todas las
secciones que la industria recomienda (LLM sampling, tools, memory
tiers, guardrails, budget, stop conditions, sub-agents, observability,
permissions, etc.).

## Cuándo activar

crear agente, nuevo agente, agente personalizado, agent config,
manifest del agente, hacer un agente, definir agente, configurar
agente, validar agente, clonar agente, agent template

## Instrucciones

### 1. Estructura del identity_dir de un agente

Cada agente vive en su propio directorio bajo
`~/.local/share/neurox/identity/<nombre>/`. El manifest es
`<nombre>.json` y los archivos referenciados viven al lado:

```
~/.local/share/neurox/identity/
├── Default.json              ← manifest del agente (obligatorio)
├── template.json             ← esqueleto base (para clonar)
├── agent.schema.json         ← JSON Schema (referenciado por $schema)
├── prompt.md                 ← system prompt
├── comportamiento.md           ← harness (reglas, contexto)
├── skills/                   ← directorio de skills
│   ├── crear-agentes.md      ← esta skill
│   └── ...
├── memory/                   ← memory tiers (opcional)
│   ├── long-term.jsonl       ← memory.long_term.path
│   └── episodes.jsonl         ← memory.episodic.path
└── facts.yaml                ← memory.semantic.path
```

### 2. Manifest v2 — todos los campos

Schema de referencia en `agent.schema.json` (`$schema` apunta ahí).
El manifest es **una sola fuente de verdad** — todos los paths y
config vienen de él, no se hardcodean en código.

#### 2.1. `meta` (obligatorio)

Identidad del agente. Lo que ve el operador y la UI.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | string (slug) | Identificador único (lowercase, sin espacios). Ej. `"developer"`. |
| `name` | string | Nombre corto, cómo aparece en logs. |
| `display_name` | string | Nombre que ve el usuario en la UI. |
| `description` | string | Descripción corta del propósito. |
| `tags` | array<string> | Tags para filtrado / búsqueda (ej. `["code", "no-restrictions"]`). |
| `author` | string | Quién lo creó. |
| `created_at` | string (ISO 8601) | Fecha de creación. |
| `updated_at` | string (ISO 8601) | Última modificación. |
| `extends` | string \| null | Nombre de otro agente del que heredar config. |

#### 2.2. `files` (obligatorio)

Paths a los archivos del agente. Todos relativos al `identity_dir`.

| Campo | Tipo | Descripción |
|---|---|---|
| `prompt` | string | Path al system prompt (default `prompt.md`). |
| `harness` | array<string> | Paths a archivos de harness, concatenados en orden. |
| `skills_dir` | string | Path al directorio de skills (default `skills/`). |

**Harness** = reglas + contexto persistente que se inyectan en cada
turno. Mantener corto, conciso, en español. Un archivo por concern
(comportamiento, workspace, conocimiento). Sin prefijos redundantes.

#### 2.3. `llm` (obligatorio)

| Campo | Tipo | Descripción |
|---|---|---|
| `provider` | string \| null | ID del provider (ej. `"anthropic"`, `"openai"`, `"minimax"`). |
| `model` | string \| null | Model ID (ej. `"claude-sonnet-4-5"`, `"gpt-5"`, `"MiniMax-M3"`). |
| `sampling.temperature` | number (0-2) \| null | Default upstream. 0.2 recomendado para agents. |
| `sampling.top_p` | number (0-1) \| null | Nucleus sampling. |
| `sampling.top_k` | int \| null | Top-k sampling. |
| `sampling.max_tokens` | int \| null | Default 4096. |
| `sampling.stop_sequences` | array<string> | Strings donde parar. |
| `sampling.seed` | int \| null | Para reproducibilidad. |
| `sampling.frequency_penalty` | number \| null | Penalizar repetición. |
| `sampling.presence_penalty` | number \| null | Penalizar topics repetidos. |

#### 2.4. `tools` (obligatorio)

| Campo | Tipo | Descripción |
|---|---|---|
| `policy.type` | enum: `allow_all` \| `allowlist` \| `denylist` | Filtro base. |
| `policy.allowlist` | array<string> \| null | Tools permitidas (si type=allowlist). |
| `policy.denylist` | array<string> \| null | Tools bloqueadas (si type=denylist). |
| `config` | object | Config per-tool (ej. `generate_video.default_duration_secs: 6`). |

#### 2.5. `mcp_servers`

Lista de servidores MCP. Cada uno es un objeto:

| Campo | Tipo | Descripción |
|---|---|---|
| `name` | string | Identificador. |
| `transport` | enum: `stdio` \| `http` \| `sse` | Tipo de transporte. |
| `command` | string? | Para stdio: comando a ejecutar. |
| `args` | array<string> | Args del comando. |
| `url` | string? | Para http/sse: endpoint URL. |
| `auth.type` | enum: `none` \| `bearer` \| `basic` \| `oauth` | Tipo de auth. |
| `auth.token_env` | string? | Env var con el token. |
| `env` | object | Env vars extra. |

#### 2.6. `memory` (4 tiers)

| Tier | Tipo | Default path | Formato |
|---|---|---|---|
| `short_term` | `in_memory` | — | Lista en memoria (max_messages). |
| `long_term` | `file` | `memory/long-term.jsonl` | JSONL: una entrada JSON por línea. |
| `episodic` | `file` | `memory/episodes.jsonl` | JSONL: un episodio JSON por línea. |
| `semantic` | `file` | `facts.yaml` | YAML con `facts: []` (id, type, content, active). |

#### 2.7. `guardrails`

Validación input/output. Lista de reglas con `type` y config per-tipo.

**Input** soportado: `max_length` (con `max_chars`), `regex_match`, `regex_reject`, `no_secrets`, `blocklist`, `custom`.
**Output** soportado: `max_length`, `no_secrets`, `no_pii`, `json_schema` (con `schema`), `regex_match`, `blocklist`, `custom`.

#### 2.8. `output`

| Campo | Tipo | Descripción |
|---|---|---|
| `type` | enum: `text` \| `json_schema` | Tipo de output. |
| `schema` | object? | JSON Schema (solo si type=json_schema). El output se valida contra esto. |

#### 2.9. `streaming`

| Campo | Tipo | Default | Descripción |
|---|---|---|---|
| `enabled` | bool | `true` | Si streamea las respuestas del LLM. |
| `chunk_size` | int? | null | Tokens por chunk (null = provider default). |

#### 2.10. `budget`

| Campo | Tipo | Descripción |
|---|---|---|
| `max_tokens_per_run` | int? | Tokens máximos por invocación. |
| `max_runtime_secs` | int? | Segundos máximos. |
| `cost_limit_usd` | number? | Costo máximo en USD. |

#### 2.11. `stop_conditions`

| Campo | Tipo | Default | Descripción |
|---|---|---|---|
| `max_iterations` | int | 10 | Máximo de iteraciones del loop de tools. |
| `max_tool_calls` | int? | null | Máximo de llamadas a tools. |
| `max_duration_secs` | int? | null | Duración máxima total. |

#### 2.12. `sub_agents`

Sub-agents a los que el agent puede delegar. Cada uno:

| Campo | Tipo | Descripción |
|---|---|---|
| `name` | string | Identificador. |
| `role` | string? | Descripción corta (ej. `"code-reviewer"`). |
| `instructions` | string | System prompt del sub-agent. |
| `tools` | array<string> | Tools que puede usar (vacío = hereda del padre). |
| `model_override` | string? | Override del model del padre. |
| `handoff_description` | string? | Cuándo delegar a este sub-agent. |

#### 2.13. `permissions.sandbox`

| Campo | Tipo | Default | Descripción |
|---|---|---|---|
| `writable_paths` | array<string> | `[]` | Paths donde el agent puede escribir. |
| `readable_paths` | array<string> | `[]` | Paths donde el agent puede leer. |
| `network_access` | bool | `true` | Puede hacer HTTP requests. |
| `process_spawn` | bool | `true` | Puede ejecutar sub-procesos. |
| `memory_limit_mb` | int? | null | Límite de memoria. |
| `cpu_limit_pct` | int? | null | Límite de CPU (1-100). |

#### 2.14. `observability`

| Campo | Tipo | Default | Descripción |
|---|---|---|---|
| `log_level` | enum: `trace` \| `debug` \| `info` \| `warn` \| `error` | `info` | Nivel del logger. |
| `trace` | bool | `false` | Grabar prompt/response completos (puede contener datos del usuario). |
| `metrics` | bool | `false` | Emitir métricas de tokens/cost/latency por run. |

### 3. Procedimiento para crear un agente nuevo

1. **Copiar el template base**:
   ```bash
   cp -r ~/.local/share/neurox/identity/Default \
         ~/.local/share/neurox/identity/MiAgente
   ```
2. **Editar `MiAgente.json`**: completar `meta` (id, name, display_name, description, tags, author, fechas). Cambiar `meta.id` (slug) y `meta.name`. Si no hereda de Default, dejar `extends: null`.
3. **Escribir `prompt.md`**: el rol específico del agente. Sé conciso, en español. Usa Markdown si ayuda a la legibilidad.
4. **Editar los harness en `files.harness[]`**: ajustar el comportamiento, contexto del workspace, conocimiento persistente. Mantener cortos.
5. **Configurar LLM** (si el agente usa un model/provider distinto): `llm.provider` y `llm.model`. Ajustar `llm.sampling` según el caso de uso (temperature baja para agents deterministas, alta para creativos).
6. **Definir tools policy**:
   - `allow_all` para agentes sin restricciones (default para tests)
   - `allowlist` con las tools específicas que el agente puede usar
   - `denylist` con las tools que NO puede usar
7. **Configurar `tools.config`** con defaults per-tool (ej. `generate_video.default_duration_secs: 6`).
8. **Configurar permissions.sandbox** según el nivel de acceso deseado.
9. **Ajustar budget / stop_conditions** según criticidad.
10. **Validar con el JSON Schema**:
    ```bash
    python3 -c "
    import json, jsonschema
    schema = json.load(open('agent.schema.json'))
    doc = json.load(open('MiAgente.json'))
    jsonschema.validate(doc, schema)
    "
    ```
11. **Registrar el agente en `daemon config.yaml`** (`~/.config/neurox/config.yaml`) bajo `session_agents.agents`.
12. **Reiniciar el daemon** y probar end-to-end con `curl` o desde la web.

### 4. Procedimiento para clonar de un agente existente

```bash
# 1. Copiar todo el identity_dir
cp -r ~/.local/share/neurox/identity/Default \
      ~/.local/share/neurox/identity/Developer

# 2. Editar Developer.json — cambiar meta.id, meta.name, meta.display_name
# 3. Ajustar prompt.md y harness/* al nuevo rol
# 4. Opcional: usar extends: "default" en meta para heredar config base
#    y solo override lo que cambia
```

Con `extends`, los campos no especificados se heredan del padre. Esto
evita duplicar config entre agentes similares.

### 5. Checklist antes de producción

- [ ] `meta.id` es slug único (no choca con otro agente)
- [ ] `meta.display_name` legible para el usuario final
- [ ] `prompt.md` específico del rol, no genérico
- [ ] Harness files cortos (< 1KB cada uno)
- [ ] `tools.policy` restrictivo (allowlist si es posible)
- [ ] `permissions.sandbox` con limits razonables
- [ ] `budget.max_runtime_secs` configurado (ej. 300-600)
- [ ] `stop_conditions.max_iterations` razonable (5-20)
- [ ] Validado con JSON Schema
- [ ] Registrado en daemon config
- [ ] Test end-to-end con un caso real del agente

## Tools preferidas

- read_file
- glob
- grep
- write_file
