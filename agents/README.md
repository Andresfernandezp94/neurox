# Neurox Agents

Plantillas versionadas de agentes para Neurox. Cada subdirectorio es
un agente completo, listo para copiarse a
`~/.local/share/neurox/identity/<nombre>/`.

## Estructura

```
agents/
└── Default/                      ← plantilla base (sin identidad específica)
    ├── Default.json               ← manifest del agente (schema_version 2)
    ├── template.json              ← esqueleto (mismo shape, campos vacíos)
    ├── agent.schema.json          ← JSON Schema (referenciado por $schema)
    ├── prompt.md                  ← system prompt
    ├── comportamiento.md            ← harness: reglas + contexto persistente
    └── skills/                    ← skills del agente (Anthropic Skills-style)
        └── crear-agentes.md       ← skill: cómo crear agentes nuevos
```

## Cómo usar

Para crear un agente nuevo:

1. **Copiar la plantilla**:
   ```bash
   cp -r agents/Default ~/.local/share/neurox/identity/default
   ```

2. **Editar `default/Default.json`**:
   - `meta.id` (slug único, ej. `"code-reviewer"`)
   - `meta.name`, `meta.display_name`, `meta.description`
   - `meta.tags` (categorización)
   - `meta.author`, fechas

3. **Adaptar `prompt.md`**: rol específico del agente.

4. **Editar los harness** (`comportamiento.md` y los que se agreguen
   en `files.harness[]`): reglas + contexto del agente.

5. **Configurar tools**: `tools.policy` (allowlist si querés
   restrictivo), `tools.config` con defaults per-tool.

6. **Validar contra el schema**:
   ```bash
   python3 -c "
   import json, jsonschema
   schema = json.load(open('~/.local/share/neurox/identity/default/agent.schema.json'))
   doc = json.load(open('~/.local/share/neurox/identity/default/default.json'))
   jsonschema.validate(doc, schema)
   "
   ```

7. **Registrar en daemon** (`~/.config/neurox/config.yaml`):
   ```yaml
   session_agents:
     agents:
       default:
         command: /home/andres_fernandez/.local/bin/agent
         args:
           - --id
           - default
           - --identity-dir
           - /home/andres_fernandez/.local/share/neurox/identity/default
         idle_timeout_secs: 1800
   ```

8. **Reiniciar el daemon** y probar.

## Schema v2

El manifest sigue la versión 2 del schema (`agent.schema.json`).
Campos principales:

- `meta` — identidad del agente
- `files` — paths a prompt/harness/skills
- `llm` — provider, model, sampling (temperature, top_p, top_k, etc.)
- `tools` — policy (allow_all/allowlist/denylist) + per-tool config
- `mcp_servers` — servidores MCP
- `memory` — short_term, long_term, episodic, semantic
- `guardrails` — input/output validation
- `output` — text o json_schema
- `streaming` — chunked output
- `budget` — max_tokens/runtime/cost
- `stop_conditions` — max_iterations, max_tool_calls
- `sub_agents` — delegation
- `permissions.sandbox` — writable/readable paths, network, process
- `observability` — log_level, trace, metrics

Para una guía completa de cada campo, ver la skill
`Default/skills/crear-agentes.md`.
