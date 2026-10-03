# INDEXING — Convenciones de Prefijos y Estados del SOT

> **Archivo canónico** que define cómo nombrar y organizar documentos en el SOT de `neurox` para que sea **buscable eficientemente** por humanos y agentes.
>
> Versión: 1.5 (2026-08-29) | Status: active | Owner: todos

## Propósito

El SOT de `neurox` (carpeta `.sdd/`) está diseñado para ser **consumido por agentes IA** además de humanos. Para que la búsqueda sea eficiente:

1. **Cada archivo tiene un prefijo** que indica su tipo
2. **Cada documento tiene un estado** y sub-estado que indica en qué fase está
3. **Cada sub-estado tiene steps** explícitos que permiten tracking granular

```bash
# Encontrar épicas en propuesta
ls .sdd/changes/EP-*/

# Encontrar épicas in-progress
rg -l "^\*\*Status\*\*: in-progress" .sdd/

# Encontrar features bloqueadas
rg -l "^\*\*Status\*\*: blocked" .sdd/

# Reporte de progreso por estado
rg "^\*\*Status\*\*:" .sdd/ | sort | uniq -c | sort -rn
```

> **Ver también**: [methodologies/method-07-estados-y-steps.md](./methodologies/method-07-estados-y-steps.md) para la convención completa de estados y sub-estados.

## Sistema de prefijos

### Tabla maestra

| Prefijo | Tipo de documento | Ubicación | Numeración | Ejemplo |
|---------|------------------|-----------|------------|---------|
| `gov-` (UPPERCASE file) | Governance (constitution, glossary, ecosystem-map, testing) | `.sdd/` root | estable | `CONSTITUTION.md`, `GOV-STRUCTURE.md`, `GLOSSARY.md` |
| `method-` | Metodología operativa (workflow, docs, código, PRs, releases, git, estados) | `.sdd/methodologies/` | `method-NN-<slug>.md` (2 dígitos) | `method-01-flujo-de-trabajo.md` |
| `audit-` | Auditoría (security, perf, accessibility) | `.sdd/auditorias/` (legacy — directorio borrado en v1.1) | `audit-YYYY-MM-DD-<tipo>.md` | (no usado actualmente) |
| `epic-` (legacy) | Épica archivada single-file | `.sdd/epicas/` (legacy) | `epic-EP-NNNN-<slug>.md` | `epic-EP-0001-admin-web-ui/` |
| `change-` (legacy) | Cambio en propuesta (épica abierta) | `.sdd/changes/` (legacy) | `change-EP-NNNN-<slug>.md` | (no usado — usamos folder) |
| `fix-` (legacy) | Bug o mejora (follow-up) | `.sdd/fixes/` (legacy — directorio borrado en v1.1) | `fix-NNN-<slug>.md` | (no usado) |
| `release-` (legacy) | Notas de release | `.sdd/releases/` (legacy) | `release-vX.Y.Z.md` | (no usado) |
| `adr-` (UPPERCASE file) | Architecture Decision Record | `.sdd/decisions/` | `ADR-NNNN-<slug>.md` (4 dígitos) | `ADR-0001-multi-agent-subprocess-architecture.md` |
| `template-` (legacy) | Plantilla single-file (deprecated — usar `templates/ep-template/`) | `.sdd/templates/` | uno por tipo | `template-epic-proposal.md` |
| `doc-` (legacy) | Documento misc | `.sdd/misc/` o `.sdd/notes/` (legacy — borrado en v1.1) | `doc-<slug>.md` | (no usado) |

> **Convención v1.3** (enforcement): los governance docs (gov-*) y los ADRs
> (adr-*) viven en el SOT root o `.sdd/governance/` con filename en
> UPPERCASE. Los EP templates (`proposal.md`, `design.md`, `tasks.md`,
> `requirements.md`) y los method-*.md son lowercase.

### Reglas de numeración

- **`method-NN`**: 2 dígitos zero-padded (01, 02, ..., 99). Orden de lectura determina el número.
- **`fix-NNN`**: 3 dígitos zero-padded (001, 002, ..., 999). Asignar correlativamente al crear.
- **`adr-NNNN`**: 4 dígitos zero-padded (0001, 0002, ...). Convención estándar.
- **`EP-NNNN`**: 4 dígitos zero-padded (0001, 0002, ...). **Continua, correlativa, sin gaps**.
- **`audit-YYYY-MM-DD`**: fecha de la auditoría.

### Estados y sub-estados

Ver [methodologies/method-07-estados-y-steps.md](./methodologies/method-07-estados-y-steps.md) para la convención completa. Resumen:

- **13 estados top-level**: `draft`, `proposed`, `approved`, `in-progress`, `in-review`, `changes-requested`, `merged`, `done`, `closed`, `blocked`, `paused`, `rejected`, `superseded`
- **20 sub-estados** con el formato `estado:subestado` (ej. `in-progress:implementation`, `in-review:pr-open`)
- **Cada sub-estado tiene un checklist de steps** explícitos

### Estructura de un archivo con prefijo y estado

```markdown
# <Título descriptivo>

> **ID**: <tipo>-<número>
> **Type**: epic | fix | method | audit | adr | template | doc
> **Status**: <estado>:<sub-estado>          # ej: "in-progress:implementation"
> **Created**: YYYY-MM-DD
> **Updated**: YYYY-MM-DD
> **Owner**: <nombre>
> **Reviewers**: [@user1, @user2]
> **Related**: [link a otros docs], [link a issues]

<contenido>

## Steps

### in-progress:implementation
- [ ] Step 1
- [ ] Step 2
```

> **Nota**: el frontmatter liviano (entre `>`) es legible en Markdown plano y parseable por agentes. No usamos YAML para mantener simplicidad.

## Estructura de carpetas del SOT

```
.sdd/                                  ← SOT del workspace neurox (entry point)
├── README.md                           ← project intro / TL;DR
├── INDEXING.md                         ← este archivo (convenciones de prefijos)
├── CONVENTION.md                       ← @/ path convention
├── STATE.md                            ← active EPs + last-numbered + next-free
├── CONSTITUTION.md                     ← principios globales (G1..G11)
├── GOV-STRUCTURE.md                    ← mapa de jerarquías del SOT
├── GLOSSARY.md                         ← vocabulario canónico
├── GOVERNANCE.md                       ← handbook único (workflow, SOT, EPs, etc.)
├── AGENTS.md                           ← entry point para agentes IA
│
├── changes/                            ← Épicas ABIERTAS (proposal → design → tasks → specs)
│   ├── EP-0007-sot-governance-hardening/
│   │   ├── proposal.md
│   │   ├── design.md
│   │   ├── tasks.md
│   │   └── specs/
│   │       └── EP-0007-01-sot-root/
│   │           └── requirements.md
│
├── archived/                           ← Épicas CERRADAS (inmutable) + REGISTRO.md
│   ├── REGISTRO.md
│   ├── EP-0001-daemon-mcps/       (cerrada: el subsistema de MCPs se eliminó)
│   ├── EP-0002-voice-web-ui/      (cerrada: voice se eliminó)
│   ├── EP-0003-agentic-llm-robustness/
│   ├── EP-0004-multi-agent-live-switch/  ← (legacy ID: EP-2026-08-15)
│   ├── EP-0005-tools-engine-integration/ ← (legacy ID: EP-2026-08-19)
│   └── EP-0006-daemon-refactor-consolidation/  ← (legacy ID: EP-0004)
│
├── decisions/                          ← ADRs cross-repo (ADR-NNNN-slug.md, 4 dígitos, UPPERCASE)
│   ├── README.md
│   ├── ADR-0001-multi-agent-subprocess-architecture.md
│   └── ADR-0002-streaming-endpoint-routing.md
│
├── methodologies/                      ← Convenciones operativas (method-NN-slug.md, 2 dígitos)
│   ├── method-README.md
│   ├── method-01-flujo-de-trabajo.md
│   ├── method-02-buenas-practicas-documentacion.md
│   ├── method-03-buenas-practicas-codigo.md
│   ├── method-04-workflow-prs.md
│   ├── method-05-checklist-release.md
│   ├── method-06-convenciones-git.md
│   └── method-07-estados-y-steps.md
│
├── templates/                          ← Plantillas para EPs
│   └── ep-template/                     ← folder canónico
│       ├── proposal.md
│       ├── design.md
│       ├── tasks.md
│       └── spec-requirements.md
│
├── security/                           ← Proceso de seguridad + MANIFEST
│   ├── README.md
│   ├── MANIFEST.md
│   ├── process.md
│   ├── bin/
│   ├── lib/
│   ├── modes/
│   └── templates/
│
├── research/                           ← Notas de research (framework-agnostic)
│   └── frameworks-ingenieria-2026.md
│
└── bin/                                ← Scripts de gestión del SOT
    ├── close-epic.sh
    ├── lint.sh                          ← SOT consistency validator (EP-0007 R-010)
    ├── list-status.sh
    ├── next-step.sh
    ├── start-epic.sh
    └── summary.sh
```

> **Convención v1.3 (migración completada)**: el plan original v1.0 preveía
> subdirectorios en español (`governance/`, `metodologias/`, `decisiones/`,
> `auditorias/`, `epicas/`, `cambios/`, `fixes/`, `releases/`). La
> convención actual usa **inglés** (más estándar para tooling, `rg`, etc.).
> Los governance docs (gov-*) y los ADRs (adr-*) usan filename en UPPERCASE.
> La migración de los prefijos legacy (`audit-`, `epic-`, `change-`,
> `fix-`, `release-`, `doc-`) se hizo en v1.1; las refs en los docs se
> actualizaron en v1.2 (date-based EPs) y v1.3 (UPPERCASE).

## Búsquedas eficientes para agentes

### Por tipo de documento

```bash
# Todas las metodologías
ls .sdd/methodologies/method-*.md

# Todas las ADRs
ls .sdd/decisions/ADR-*.md

# Todas las épicas abiertas
ls .sdd/changes/

# Todas las épicas cerradas
ls .sdd/archived/

# Plantillas
ls .sdd/templates/ep-template/

# Documentos raíz
ls .sdd/*.md
```

### Por estado (dentro de un directorio)

```bash
# Épicas activas (no cerradas)
rg -l "^\*\*Status\*\*: in-progress" .sdd/changes/

# Metodologías activas
ls .sdd/methodologies/

# Auditorías de seguridad (legacy, no usado actualmente)
ls .sdd/security/
```

### Por EP o tema

```bash
# Todo lo relacionado con EP-0001
rg -l "EP-0001" .sdd/

# Todo lo de seguridad
rg -l -i "security|seguridad" .sdd/

# Hallar un ADR por tema (ej. autenticación)
rg -l -i "auth|jwt" .sdd/decisions/
```

### Por relación con un archivo de código

```bash
# Encontrar épicas/ADRs que mencionan un módulo
rg -l "src/api/sessions.ts" .sdd/
```

## Convenciones de nombrado

### Slugs

- **Solo kebab-case**: `method-01-flujo-de-trabajo.md`
- **Sin acentos**: `fix-001-sidebar-navid-mismatch.md` (no "ñ" ni "í")
- **Sin caracteres especiales**: solo `[a-z0-9-]`
- **Máximo 60 chars** para el nombre completo del archivo

### IDs

- **Prefijo + número + slug**: `method-01-flujo-de-trabajo.md`
- **Numeración correlativa** dentro de cada prefijo
- **Sin resetear** la numeración entre versiones

## Reglas de inmutabilidad

- **`gov-*`**: solo via ADR + proposal. Cambios requieren workflow formal.
- **`adr-*`**: inmutables una vez aceptados. Superseden con nuevos ADRs.
- **`method-*`**: editables (se mantienen al día).

## Versionado

- **Major bump del SOT** (1.0 → 2.0): cuando se cambia el sistema de prefijos
- **Minor bump** (1.0 → 1.1): cuando se agregan nuevos prefijos
- **Patch bump** (1.0.0 → 1.0.1): correcciones de typos o clarificaciones
- **Tracking**: el header de este archivo incluye la versión

## Anti-patterns

- ❌ **Archivos sin prefijo** (no se pueden buscar eficientemente)
- ❌ **Prefijos inconsistentes** (mezclar `audit-` con `security-audit-`)
- ❌ **Slugs con caracteres no-ASCII** (rompen búsquedas con `rg`)
- ❌ **IDs duplicados** (2 archivos con el mismo prefijo+número)
- ❌ **Gaps en numeración** (method-01, method-03 sin method-02)
- ❌ **Prefijos demasiado largos** (más de 8 caracteres)

## Version

- **1.0** (2026-08-08) — inicial
  - 11 prefijos definidos (gov, method, audit, epic, change, fix, release, changelog, adr, template, doc)
  - Estructura de 9 subdirectorios
  - Convenciones de búsqueda con `ls` + `rg`
- **1.1** (2026-08-15) — align con layout real
  - `changes/` en inglés (era `cambios/` planeado)
  - `methodologies/` en inglés (era `metodologias/`)
  - `decisions/` en inglés (era `decisiones/`)
  - Removidos del plan los dirs no existentes: `governance/`, `auditorias/`, `epicas/`, `fixes/`, `releases/`
  - `archived/REGISTRO.md` documenta EPs cerradas
- **1.2** (2026-08-29) — strict sequential numeric EPs
  - Date-based EP naming **REMOVED**. Numeric `EP-NNNN` only.
  - Migration: `EP-2026-08-15-multi-agent-live-switch` → `EP-0004-multi-agent-live-switch`
  - Migration: `EP-2026-08-19-tools-engine-integration` → `EP-0005-tools-engine-integration`
  - Migration: `EP-0004-daemon-refactor-consolidation` → `EP-0006-daemon-refactor-consolidation`
  - Frontmatter gains `**Legacy ID**:` field for renumbered EPs.
  - `bin/start-epic.sh` updated to reject date-based slugs.
  - `archived/REGISTRO.md` gets a `Legacy ID` column.
  - Added `GOV-STRUCTURE.md` (mapa de jerarquías del SOT).
  - Next free: `EP-0007`.
- **1.3** (2026-08-29) — strict UPPERCASE for governance + ADR
  - **Migration to UPPERCASE** for all cross-cutting governance docs:
    - `constitution.md` → `CONSTITUTION.md`
    - `gov-structure.md` → `GOV-STRUCTURE.md`
    - `decisions/adr-NNNN-*.md` → `decisions/ADR-NNNN-*.md`
  - **Convention**: from v1.3, all governance + ADR docs MUST be
    UPPERCASE. EP templates (`proposal.md`, `design.md`, `tasks.md`,
    `requirements.md`) stay lowercase.
  - Prefijos `gov-` y `adr-` ahora son UPPERCASE; el resto del nombre
    del archivo sigue en kebab-lowercase (`CONSTITUTION.md`,
    `GOV-STRUCTURE.md`, `ADR-0001-...md`).
- **1.4** (2026-08-29) — `DOCS-INDEX.md` canonical catalog
  - Nuevo governance doc en `.sdd/` raíz: [`DOCS-INDEX.md`](./DOCS-INDEX.md).
  - Es el **catálogo canónico de TODA la documentación del workspace**
    (governance + module docs + decisions + postmortems + templates +
    security + research + EPs activas + EPs archivadas).
  - Si un archivo `.md` existe en `.sdd/` o `docs/` y no aparece en
    `DOCS-INDEX.md`, está huérfano (TODO: regenerador automático).
  - Mantenimiento: manual por ahora; auditar al cerrar cada EP.
- **1.5** (2026-08-29) — G12 + G13 + agent naming policy
  - **G12** (CONSTITUTION): jerarquía SOT > Código > Doc. Cualquier cambio
    debe propagar a la doc en el mismo PR.
  - **G13** (CONSTITUTION): el único `agent_id` válido es `default`. NO
    usar `adan`, `eva`, `neurox_default`, `default_agent`, ni nombres
    propios. Display name `Default` (CapitalCase) sí permitido.
  - Nueva metodología [`method-08-doc-update-mandatory.md`](./methodologies/method-08-doc-update-mandatory.md)
    para enforce G12.
  - **EP-0009** (purga de nombres de agentes no canónicos) ejecuta esto:
    sub-repos sin `.sdd/` ni `CHANGELOG.md`, solo `.sdd/` root canónico.