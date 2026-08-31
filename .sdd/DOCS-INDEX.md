# DOCS-INDEX — Catálogo de la documentación del workspace

> **Índice canónico** de **toda** la documentación del workspace
> `neurox`. Si un archivo `.md` no aparece acá, no debería existir.
>
> **Convención v1.4** (agregada en EP-0008): toda doc — governance,
> operación, módulo, decisión, postmortem, template — vive en una de
> las ubicaciones listadas abajo. El SOT (`.sdd/`) es el single source
> of truth; este archivo es el **mapa** de ese SOT.
>
> **Audiencia**: agentes IA y humanos que entran al repo por primera
> vez. Usar como punto de partida antes de buscar en otra parte.
>
> **Status**: active | **Owner**: todos | **Última actualización**: 2026-08-29
>
> **Mantenimiento**: por ahora manual. Migrar a regenerador automático
> (`bin/docs-index.sh`) en EP futura si la deuda de sincronización se
> vuelve problema.

---

## 1. Governance (`.sdd/` raíz — UPPERCASE)

Documentos que definen **cómo** se trabaja en el workspace.

| Doc | Propósito |
|-----|-----------|
| [`CONSTITUTION.md`](./CONSTITUTION.md) | Principios G1..G11 del ecosistema neurox |
| [`GOVERNANCE.md`](./GOVERNANCE.md) | Handbook único: workflow, SOT, quality gates, escalación |
| [`GLOSSARY.md`](./GLOSSARY.md) | Vocabulario canónico (EP, ADR, SOT, status, etc.) |
| [`INDEXING.md`](./INDEXING.md) | Convenciones de prefijos, naming, estados |
| [`GOV-STRUCTURE.md`](./GOV-STRUCTURE.md) | Mapa de jerarquías del SOT |
| [`CONVENTION.md`](./CONVENTION.md) | Path convention `@/` → workspace root |
| [`AGENTS.md`](./AGENTS.md) | Entry point de 5 pasos para agentes IA |
| [`STATE.md`](./STATE.md) | Estado machine-readable (épicas activas, próxima libre) |
| [`DOCS-INDEX.md`](./DOCS-INDEX.md) | **Este archivo** — catálogo completo |

## 2. Decisiones arquitectónicas (`.sdd/decisions/`)

ADRs (Architecture Decision Records) — inmutables una vez aceptados.

| ADR | Título | EP relacionada |
|-----|--------|----------------|
| [`ADR-0001-multi-agent-subprocess-architecture.md`](./decisions/ADR-0001-multi-agent-subprocess-architecture.md) | Multi-agent: one subprocess per session per agent | EP-0004 |
| [`ADR-0002-streaming-endpoint-routing.md`](./decisions/ADR-0002-streaming-endpoint-routing.md) | Streaming ya no rutea via llmd sino via `dispatch_to_agent` | EP-0004 |
| [`decisions/README.md`](./decisions/README.md) | Índice de ADRs y template para nuevas | — |

## 3. Methodologies (`.sdd/methodologies/`)

Workflows operativos (lowercase, `method-NN-<slug>.md`).

| Doc | Propósito |
|-----|-----------|
| [`method-01-flujo-de-trabajo.md`](./methodologies/method-01-flujo-de-trabajo.md) | Workflow diario |
| [`method-02-buenas-practicas-documentacion.md`](./methodologies/method-02-buenas-practicas-documentacion.md) | Buenas prácticas de docs |
| [`method-03-buenas-practicas-codigo.md`](./methodologies/method-03-buenas-practicas-codigo.md) | Buenas prácticas de código |
| [`method-04-workflow-prs.md`](./methodologies/method-04-workflow-prs.md) | Workflow de PRs (review, merge, squash) |
| [`method-05-checklist-release.md`](./methodologies/method-05-checklist-release.md) | Checklist de releases (T-1, T-0, T+1) |
| [`method-06-convenciones-git.md`](./methodologies/method-06-convenciones-git.md) | Convenciones de git (commits, branches) |
| [`method-07-estados-y-steps.md`](./methodologies/method-07-estados-y-steps.md) | Máquina de estados de EPs y steps |
| [`method-README.md`](./methodologies/method-README.md) | Índice de methodologies | 

## 4. Templates (`.sdd/templates/`)

Plantillas para crear nuevos documentos.

| Doc | Propósito |
|-----|-----------|
| [`templates/ep-template/proposal.md`](./templates/ep-template/proposal.md) | Plantilla de proposal de EP |
| [`templates/ep-template/design.md`](./templates/ep-template/design.md) | Plantilla de design de EP |
| [`templates/ep-template/tasks.md`](./templates/ep-template/tasks.md) | Plantilla de tasks de EP |
| [`templates/ep-template/spec-requirements.md`](./templates/ep-template/spec-requirements.md) | Plantilla de spec/requirements |

## 5. Security (`.sdd/security/`)

Procesos de seguridad agent-agnostic.

| Doc | Propósito |
|-----|-----------|
| [`security/README.md`](./security/README.md) | Entry point del módulo de seguridad |
| [`security/process.md`](./security/process.md) | Proceso end-to-end |
| [`security/MANIFEST.md`](./security/MANIFEST.md) | Manifiesto auto-actualizado de procesos activos |
| [`security/modes/audit.md`](./security/modes/audit.md) | Modo: auditoría de seguridad |
| [`security/modes/threat-model.md`](./security/modes/threat-model.md) | Modo: threat modeling |
| [`security/modes/incident.md`](./security/modes/incident.md) | Modo: respuesta a incidentes |
| [`security/modes/disclosure.md`](./security/modes/disclosure.md) | Modo: disclosure policy |
| [`security/lib/disclosure-policy.md`](./security/lib/disclosure-policy.md) | Política de disclosure |
| [`security/lib/severity-matrix.md`](./security/lib/severity-matrix.md) | Matriz de severidad |
| [`security/lib/stride-explained.md`](./security/lib/stride-explained.md) | STRIDE explicado |
| [`security/lib/owasp-asvs-mini.md`](./security/lib/owasp-asvs-mini.md) | OWASP ASVS mini |
| [`security/lib/nist-ssdf-mini.md`](./security/lib/nist-ssdf-mini.md) | NIST SSDF mini |
| [`security/lib/example-findings.md`](./security/lib/example-findings.md) | Ejemplos de findings |
| [`security/templates/audit-proposal.template.md`](./security/templates/audit-proposal.template.md) | Template: proposal de audit |
| [`security/templates/audit-design.template.md`](./security/templates/audit-design.template.md) | Template: design de audit |
| [`security/templates/audit-report.template.md`](./security/templates/audit-report.template.md) | Template: report de audit |
| [`security/templates/finding.template.md`](./security/templates/finding.template.md) | Template: finding |
| [`security/templates/threat-model.template.md`](./security/templates/threat-model.template.md) | Template: threat model |
| [`security/templates/incident-postmortem.template.md`](./security/templates/incident-postmortem.template.md) | Template: postmortem de incidente |

## 6. Research (`.sdd/research/`)

Documentación framework-agnostic.

| Doc | Propósito |
|-----|-----------|
| [`research/frameworks-ingenieria-2026.md`](./research/frameworks-ingenieria-2026.md) | Frameworks de ingeniería comparados (2026) |

## 7. Épicas activas (`.sdd/changes/`)

Una sola a la vez (ver [`STATE.md`](./STATE.md)).

| EP | Título | Status |
|----|--------|--------|
| [`EP-0008-centralize-module-docs/`](./changes/EP-0008-centralize-module-docs/) | Centralize module docs into the meta-repo | `draft:proposal` |

## 8. Épicas archivadas (`.sdd/archived/`)

Ver [`archived/REGISTRO.md`](./archived/REGISTRO.md) para el índice
cronológico completo. 7 EPs cerradas a la fecha.

## 9. Module docs (`docs/<module>/`)

Documentación de cada módulo. **Único lugar donde vive la doc de módulo.**

### 9.1 `docs/daemon/`

| Doc | Propósito |
|-----|-----------|
| [`docs/daemon/README.md`](../docs/daemon/README.md) | Índice del módulo daemon |
| [`docs/daemon/architecture.md`](../docs/daemon/architecture.md) | Arquitectura interna |
| [`docs/daemon/api.md`](../docs/daemon/api.md) | API reference |
| [`docs/daemon/agents.md`](../docs/daemon/agents.md) | Sistema de agentes |
| [`docs/daemon/development.md`](../docs/daemon/development.md) | Guía de desarrollo |
| [`docs/daemon/production.md`](../docs/daemon/production.md) | Deploy a producción |
| [`docs/daemon/plugin-system.md`](../docs/daemon/plugin-system.md) | Sistema de plugins |
| [`docs/daemon/tool-plugin-bridge.md`](../docs/daemon/tool-plugin-bridge.md) | Bridge tool↔plugin |
| [`docs/daemon/migration-from-apitoken.md`](../docs/daemon/migration-from-apitoken.md) | Migración desde api-token |
| [`docs/daemon/integrations/memory.md`](../docs/daemon/integrations/memory.md) | Integración con memory MCP |
| [`docs/daemon/integrations/voice.md`](../docs/daemon/integrations/voice.md) | Integración con voice MCP |

### 9.2 `docs/agents/`

| Doc | Propósito |
|-----|-----------|
| [`docs/agents/README.md`](../docs/agents/README.md) | Índice de agentes |
| [`docs/agents/Default/prompt.md`](../docs/agents/Default/prompt.md) | Prompt base del agente Default |
| [`docs/agents/Default/comportamiento.md`](../docs/agents/Default/comportamiento.md) | Comportamiento del agente Default |
| [`docs/agents/Default/skills/crear-agentes.md`](../docs/agents/Default/skills/crear-agentes.md) | Skill: crear nuevos agentes |

### 9.3 `docs/mcp-memory/`

| Doc | Propósito |
|-----|-----------|
| [`docs/mcp-memory/postmortems/2026-08-11-memory-mcp-401-and-deploy-env-var-breakage.md`](../docs/mcp-memory/postmortems/2026-08-11-memory-mcp-401-and-deploy-env-var-breakage.md) | Postmortem: 401 + env var breakage (2026-08-11) |

### 9.4 `docs/landing/` (crear en EP-0008 T-9)

| Doc | Propósito |
|-----|-----------|
| `docs/landing/README.md` | Índice del sitio público (TBD) |
| `docs/landing/AGENTS.md` | Guía para agentes IA del landing (TBD) |

### 9.5 `docs/<module>/decisions/` (decisiones de módulo)

| Doc | Propósito |
|-----|-----------|
| [`docs/daemon/decisions/0008-tool-plugin-bridge.md`](../docs/daemon/decisions/0008-tool-plugin-bridge.md) | Decisión local del daemon: tool-plugin bridge |

### 9.6 Índice raíz (`docs/README.md`)

| Doc | Propósito |
|-----|-----------|
| [`docs/README.md`](../docs/README.md) | Índice general de module docs |

---

## Convenciones de referencia

- **Link relativo desde `.sdd/`** → `./FILE.md` (mismo dir) o `../FILE.md` (un nivel arriba).
- **Link desde `.sdd/` a `docs/`** → `../docs/<module>/FILE.md` (dos niveles arriba, porque `.sdd/` está anidado en meta-repo).
- **UPPERCASE** solo para governance docs raíz (v1.3).
- **lowercase** para todo lo demás (v1.3).

## Cómo navegar

1. **Nuevo en el repo**: arrancar por [`README.md`](./README.md) → [`AGENTS.md`](./AGENTS.md) → este índice.
2. **Buscar una EP**: este índice §7-§8 + [`STATE.md`](./STATE.md) + [`archived/REGISTRO.md`](./archived/REGISTRO.md).
3. **Buscar una decisión**: este índice §2 + §9.5.
4. **Buscar un postmortem**: este índice §9.3 (por ahora; crecerá con otros módulos).
5. **Buscar un workflow**: [`methodologies/`](./methodologies/) (índice completo en este doc §3).
6. **Auditar el SOT**: `bash .sdd/bin/lint.sh` debe pasar exit 0.

## Version

- **1.0** (2026-08-29) — inicial (creado durante EP-0008, R-NNN)
  - Catálogo completo de governance + decisions + methodologies + templates + security + research + EPs + module docs
  - 9 secciones + convenciones de referencia + flujo de navegación
  - Mantenimiento manual por ahora; regenerador automático en EP futura
