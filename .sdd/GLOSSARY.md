> **HISTORICAL**: este documento menciona `adan`/`eva` (nombres
> pre-rename). El agente canónico del workspace es `default`.
> Ver [EP-0009](../../changes/EP-0009-purge-non-default-agent-names/)
> para el rename completo. Para git history exacto:
> `git log --all --grep="adan|eva"`.

# Glosario — neurox ecosystem

Vocabulario canónico compartido entre todos los repos.
Si un término se usa en código, docs, o conversación, debe coincidir con esta definición.

## Términos

| Término | Definición |
|---------|-----------|
| **daemon** | El proceso `neurox` que corre como systemd user service. Orquesta todo. |
| **agent** | Un proceso (persistent o ephemeral) que ejecuta lógica IA. Se comunica con el daemon via JSON-RPC stdio o HTTP. |
| **persistent agent** | Agente de larga vida (e.g. `agent`). Se inicia al arrancar el daemon. |
| **ephemeral agent** | Agente on-demand para una tarea específica. Se destruye al terminar. |
| **default** | El agente que habla con el LLM. Tiene identity, memory, skills. Es el "agent" por default cuando el daemon arranca. |
| **session agent** (EP-0004) | Agente atado a un `session_id`. Cada sesión tiene su propio subprocess agent con working memory aislada. |
| **plugin** | Un binario externo (repo separado) que extiende el daemon con tools y skills. Corre como proceso separado. |
| **tool** | Una función que el agent puede invocar. Puede ser core (built-in) o plugin (HTTP proxy). |
| **skill** | Instrucciones markdown que enseñan al agent CUÁNDO y CÓMO usar tools específicas. Se activan por keyword match. |
| **manifest** | El `manifest.json` de un plugin: metadata, capabilities, tools, skills. |
| **SSD** | Spec-Driven Development. Directorio `.sdd/` con constitution, glossary, ADRs, épicas. |
| **ADR** | Architecture Decision Record. Decisión inmutable que explica el porqué. |
| **épica** | Una unidad de trabajo con proposal → design → specs → tasks. |
| **scope** | Visibilidad de una memoria: `private` (solo el agente owner), `global` (todos) o `system` (interno). |
| **agent_id** | Identificador del agente caller. Usado para ACL en el memory plugin. |
| **dynamic registration** | Mecanismo por el cual un plugin POST sus tools + skills al daemon en runtime. |
| **reconciliation** | Cuando el daemon compara las tools actuales de un plugin con las nuevas y registra/unregistra la diferencia. |
| **reconnect** | Admin trigger para que el daemon re-descubra un plugin caído (GET /tools, 2 retries). |
| **workspace** | Este repo (`neurox`). Contiene submodules de todos los repos del ecosistema. |
| **SOT** | Source of Truth. El `.sdd/` raíz de neurox es el SOT del producto. |
| **mcp** | Model Context Protocol. Wire format que el daemon usa para comunicarse con plugins externos (HTTP o stdio). |
| **streaming endpoint** | El endpoint `POST /v1/sessions/:id/messages/stream` que el cliente web usa para chat en tiempo real. Delega en el subprocess agent del `session_id` correspondiente. |
