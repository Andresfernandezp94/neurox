# SOT neurox — `.sdd/`

Entry point para **CUALQUIER agente** (humano o IA). Llegaste acá → empezá por acá.

> **Regla crítica** (v1.3): **toda la documentación vive en `.sdd/`**. Los
> governance docs raíz son UPPERCASE; los EP templates y docs de
> operación son lowercase.

## ¿Qué necesitás?

| Si querés... | Hacé esto |
|---|---|
| **Catálogo de toda la doc** del workspace | Leé [DOCS-INDEX.md](./DOCS-INDEX.md) |
| **Resumen de 1 página** del workspace | `bash bin/summary.sh` |
| **Saber cómo se trabaja** en el repo | Leé [GOVERNANCE.md](./GOVERNANCE.md) (handbook único) |
| **Si sos un agente IA** nuevo | Leé [AGENTS.md](./AGENTS.md) (entry point de 5 pasos) |
| **Ver estado actual** del SOT | `cat STATE.md` o `bash bin/summary.sh` |
| **Crear nueva épica** | `bash bin/start-epic.sh <slug-descriptivo>` |
| **Cerrar épica** (mover a `archived/`) | `bash bin/close-epic.sh <EP-NNNN>` (atómico desde EP-0007) |
| **Retomar trabajo en curso** | `bash bin/next-step.sh` |
| **Ver estado de todas las épicas** | `bash bin/list-status.sh` |
| **Verificar consistencia del SOT** | `bash bin/lint.sh` (creado en EP-0007 R-010) |
| **Auditar seguridad** | `bash security/bin/start.sh audit "<scope>"` |
| **Threat model de feature** | `bash security/bin/start.sh threat-model "<feature>"` |
| **Principios del ecosistema** | Leé [CONSTITUTION.md](./CONSTITUTION.md) |
| **Vocabulario canónico** | Leé [GLOSSARY.md](./GLOSSARY.md) |
| **Workflow diario** | Leé [methodologies/method-01-flujo-de-trabajo.md](./methodologies/method-01-flujo-de-trabajo.md) |
| **Cómo nombrar archivos** | Leé [INDEXING.md](./INDEXING.md) |
| **Máquina de estados** | Leé [methodologies/method-07-estados-y-steps.md](./methodologies/method-07-estados-y-steps.md) |

## Estructura

| Carpeta | Qué hay |
|---|---|
| `templates/` | Templates OpenSpec (proposal, design, tasks, epic) |
| `metodologias/` | 7 workflows operativos (workflow diario, docs, código, PRs, releases, git, estados) |
| `security/` | Proceso de seguridad agent-agnostic (audit, threat-model, incident, disclosure) |
| `decisions/` | ADRs (Architecture Decision Records) — inmutables |
| `research/` | Documentación de referencia (frameworks comparados) |
| `bin/` | Scripts ejecutables del flujo OpenSpec |
| `changes/` | Épicas activas (auto-creadas por `bin/start-epic.sh`) |
| `archived/` | Épicas cerradas (auto-pobladas por `bin/close-epic.sh`) |
| `STATE.md` | Estado machine-readable del SOT |

## Reglas duras

1. **No crees archivos a mano** en `changes/` o `archived/`. Usá `bin/start-epic.sh` y `bin/close-epic.sh`.
2. **Numeración correlativa** — los scripts calculan el siguiente número. No lo pongas vos.
3. **No edites el frontmatter de `Status` a mano** — actualizalo según avanza el trabajo.
4. **Si no entendés algo**, leé `CONSTITUTION.md` y `GLOSSARY.md` antes de preguntar.
