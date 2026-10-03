# SOT — Estructura y jerarquías

> **Cómo se organiza el Source of Truth** de neurox: épicas, specs,
> fases, ADRs y cross-cutting concerns. Este doc es el mapa; los
> prefijos y estados detallados viven en [`INDEXING.md`](./INDEXING.md).

## Vista de árbol

```
.sdd/                                     ← SOT del workspace (meta-repo)
│
├── README.md                             intro / TL;DR
├── INDEXING.md                           prefijos de archivo + estados
├── CONVENTION.md                         path convention (@/ → workspace root)
├── STATE.md                              épicas activas + última numerada
├── STRUCTURE.md                          ← este doc (mapa de jerarquías)
├── CONSTITUTION.md                       principios globales (G1..G11) — UPPERCASE
├── GOVERNANCE.md                         handbook único de cómo trabajar — UPPERCASE
├── GOV-STRUCTURE.md                      este doc (mapa de jerarquías) — UPPERCASE
├── GLOSSARY.md                           vocabulario canónico — UPPERCASE
│
├── changes/                              ← ÉPICAS ABIERTAS (en propuesta / wip)
│   └── EP-NNNN-<slug>/                   una carpeta por épica abierta
│       ├── proposal.md                    REQUIRED: qué + por qué + criterios
│       ├── design.md                      decisiones técnicas + contratos
│       ├── tasks.md                       plan por fases (Phase 1..N) con steps
│       └── specs/                         ← solo si la EP toca varios repos
│           └── EP-NNNN-NN-<repo>/         una carpeta por repo / subsystem
│               └── requirements.md       requisitos (R-001..) con criterios
│
├── archived/                             ← ÉPICAS CERRADAS (inmutable)
│   ├── REGISTRO.md                        índice cronológico
│   └── EP-NNNN-<slug>/|EP-YYYY-MM-DD-<slug>/
│       ├── proposal.md | design.md | tasks.md | specs/...
│       └── REGISTRO.md se actualiza vía bin/close-epic.sh
│
├── decisions/                            ← ADRs cross-repo (inmutables)
│   └── adr-NNNN-<slug>.md                 4 dígitos, ej: ADR-0001-multi-agent-subprocess-architecture.md
│
├── methodologies/                        ← Convenciones operativas (editables)
│   └── method-NN-<slug>.md                2 dígitos (01..99)
│
├── templates/                            ← Plantillas para proposal / design / tasks / specs
│   ├── ep-template/                       carpeta de plantillas de EP (1 archivo por doc)
│   └── template-epic-with-states.md       plantilla monolítica de EP (con estados+steps)
│
├── research/                             ← Notas framework-agnostic
├── security/                             ← MANIFEST + procesos de seguridad
└── bin/                                  ← scripts de gestión del SOT
    ├── start-epic.sh                     crea changes/EP-NNNN-<slug>/ + numeración
    ├── close-epic.sh                     mueve changes/EP-NNNN/ → archived/ + REGISTRO.md
    ├── list-status.sh                     reporte por estado
    ├── next-step.sh                       propone el próximo step
    └── summary.sh                         resumen del SOT
```

## Jerarquía conceptual

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ÉPICA  (EP-NNNN)                                                         │
│   Unidad de trabajo top-level. Numerada correlativamente.                  │
│   Una carpeta por épica. OpenSpec-style: proposal → design → tasks.        │
│                                                                          │
│   ├─ proposal.md        qué problema + solución + criterios de aceptación │
│   ├─ design.md          decisiones técnicas (DT-N) + contratos (C-N)      │
│   ├─ tasks.md           plan por fases (Phase 1..N) con steps verificables│
│   └─ specs/             ↓ solo si la EP toca múltiples repos             │
│       │                                                                      │
│       ├─ SPEC 1  (EP-NNNN-01-<repoA>)                                       │
│       │    Una carpeta por repo / subsystem.                                 │
│       │    Contiene requirements.md con R-001.. verificables.              │
│       │                                                                      │
│       ├─ SPEC 2  (EP-NNNN-02-<repoB>)                                       │
│       │    Misma estructura. Si la EP toca N repos → N specs.             │
│       │                                                                      │
│       └─ SPEC N  ...                                                         │
└──────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────┐
│ CROSS-CUTTING                                                              │
│                                                                          │
│ ADR  (Architecture Decision Record)                                        │
│   decisions/adr-NNNN-<slug>.md     inmutable una vez aceptado.             │
│                                   4 dígitos (0001..).                       │
│                                   Decisiones que aplican a varios repos.   │
│                                                                          │
│ METHOD (Metodología operativa)                                              │
│   methodologies/method-NN-<slug>.md   2 dígitos (01..).                   │
│                                   Editable. Cómo se trabaja en el repo.   │
│                                                                          │
│ SECURITY                                                                  │
│   security/MANIFEST.md + bin/      procesos de seguridad activos.          │
│                                   Auto-actualizado por security/bin/start.sh│
└──────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────┐
│ GOVERNANCE  (root del SOT — siempre presente)                              │
│                                                                          │
│ CONSTITUTION.md  principios globales (G1..G11), heredados por sub-repos. │
│ GOVERNANCE.md    handbook único de cómo trabajar en este repo.            │
│ GOV-STRUCTURE.md este doc (mapa de jerarquías del SOT).                   │
│ GLOSSARY.md      vocabulario canónico.                                    │
│ CONVENTION.md    path convention (`@/` → workspace root).                  │
│ INDEXING.md      prefijos de archivo, estados, numeración.                │
│ STATE.md         épicas activas + última numerada + ADR vigentes.          │
│ STRUCTURE.md     ← este doc: mapa visual de jerarquías.                    │
└──────────────────────────────────────────────────────────────────────────┘
```

## Ciclo de vida de una ÉPICA

```
changes/EP-NNNN-<slug>/                archived/EP-NNNN-<slug>/
┌──────────────────────────┐            ┌──────────────────────────┐
│ draft:proposal          │            │ (read-only)              │
│   ↓ self-review         │            │                          │
│ draft:design            │ ─ close ─→ │ propuesta aceptada       │
│   ↓                    │            │ diseño cerrado            │
│ draft:tasks             │            │ tareas ejecutadas         │
│   ↓                    │            │                          │
│ proposed:review-pending │            │ Subsistemas documentados │
│   ↓ reviewer approval  │            │ en specs/ cerrados        │
│ proposed:specs-pending  │            │                          │
│   ↓                    │            │                          │
│ approved:ready-to-start │            │                          │
│   ↓                    │            │                          │
│ in-progress:implementation              │                          │
│   ↓                    │            │                          │
│ in-progress:tests      │            │                          │
│   ↓                    │            │                          │
│ in-progress:docs       │            │                          │
│   ↓                    │            │                          │
│ in-review:pr-open      │            │                          │
│   ↓                    │            │                          │
│ in-review:feedback-addressed          │                          │
│   ↓                    │            │                          │
│ in-progress:merge      │            │                          │
│   ↓                    │            │                          │
│ done:deployed / done:verified          │                          │
│   ↓                    │            │                          │
│ closed:archived        │ ─ close ─→ │ (REGISTRO.md actualizado) │
└──────────────────────────┘            └──────────────────────────┘

Estados detallados + steps: ver methodologies/method-07-estados-y-steps.md
```

## Estados resumidos (top-level)

```
draft → proposed → approved → in-progress → in-review → done → closed
                                                  │
                                                  └──→ blocked / paused / superseded / rejected
```

Sub-estados (formato `top:sub`): ver `methodologies/method-07-estados-y-steps.md`.

## Numeración

| Tipo | Formato | Dígitos | Ejemplo | Regla |
|---|---|---|---|---|
| Épica (EP) | `EP-NNNN` | 4 | `EP-0006`, `EP-2026-08-19` | correlativa, sin gaps |
| Spec (dentro de EP) | `EP-NNNN-NN-<repo>` | 2 | `EP-0001-04-sessions` | correlativa dentro de la EP |
| ADR | `adr-NNNN-<slug>` | 4 | `ADR-0001-multi-agent-subprocess-architecture.md` | correlativa |
| Método | `method-NN-<slug>` | 2 | `method-07-estados-y-steps.md` | orden de lectura |
| Audit | `audit-YYYY-MM-DD-<tipo>` | fecha | `audit-2026-08-08-security/` | fecha del audit |
| Fix | `fix-NNN-<slug>` | 3 | `fix-001-sidebar-navid-mismatch.md` | correlativa |

## Convención de prefijos (resumen)

| Prefijo | Tipo | Ubicación |
|---|---|---|
| `gov-` (UPPERCASE) | Governance | `.sdd/` (migración completada en v1.3) |
| `adr-` | Architecture Decision Record | `.sdd/decisions/` |
| `method-` | Metodología operativa | `.sdd/methodologies/` |
| `audit-` | Auditoría | `.sdd/auditorias/` (no usado todavía) |
| `epic-` | Épica archivada | `.sdd/archived/EP-NNNN-*/` |
| `change-` | Épica abierta (legacy) | `.sdd/changes/EP-NNNN-*/` |
| `fix-` | Bug o mejora | `.sdd/fixes/` (no usado) |
| `release-` | Notas de release | `.sdd/releases/` (no usado) |
| `template-` | Plantillas | `.sdd/templates/` |
| `doc-` | Doc misc | `.sdd/notes/` (no usado) |

Detalle completo: ver [`INDEXING.md`](./INDEXING.md).

## Cómo encaja esto con el meta-repo

```
neurox/  (meta-repo)
├── .sdd/                     ← SOT global (este este archivo)
│   └── changes/EP-0004-.../
│       └── specs/EP-0004-01-daemon/requirements.md
│
├── daemon/  (submodule)       ← código del daemon
│   └── .sdd/                 ← SOT local (symlinks a este .sdd/)
│       ├── decisions/        (symlink)
│       └── methods...         (symlink)
│
├── client/web/  (submodule)  ← código del cliente web
│   └── .sdd/                 ← SOT local del cliente
│
└── daemon/
    ├── core/                  (servidor HTTP + sesión + auth)
    ├── tools-engine/          (las 14 tools nativas)
    ├── crates/agent-lib/      (identity, tools_filter)
    └── agents/                (subprocess por sesión)
```

> Nota: hasta 2026-10-03 hubo un `mcps/` con `memory/`, `voice/`,
> `clickup/` y `playwright/`. Los MCPs ya no forman parte de neurox: el
> daemon no tiene subsistema de plugins y las rutas `/v1/mcps` se
> eliminaron.

Cada sub-repo hereda los principios globales (`CONSTITUTION.md`,
`GOVERNANCE.md`, GOV-STRUCTURE.md, `GLOSSARY.md`) y puede tener su
propio `.sdd/` para épicas locales.

## Paths convention

`@/` se expande al workspace root (definido en `CONVENTION.md`).
Dentro de las specs: `@/daemon/core/src/...` apunta al daemon.

## Ver también

- [`INDEXING.md`](./INDEXING.md) — prefijos y estados detallados
- [`CONVENTION.md`](./CONVENTION.md) — path convention `@/`
- [`STATE.md`](./STATE.md) — épicas activas ahora
- [`methodologies/method-07-estados-y-steps.md`](./methodologies/method-07-estados-y-steps.md) — máquina de estados
- [`methodologies/method-01-flujo-de-trabajo.md`](./methodologies/method-01-flujo-de-trabajo.md) — flujo de trabajo