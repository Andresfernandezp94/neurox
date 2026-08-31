# GOVERNANCE — neurox workspace

> **El handbook único** que explica **cómo se trabaja** en este repositorio.
> Concreto, sin huecos, escalable. **Lee esto primero** — todo lo demás
> (CONSTITUTION, GLOSSARY, INDEXING, methodologies/*) está linkeado desde acá.
>
> Versión: 1.0 (2026-08-29) | Status: active | Owner: @andres_fernandez
>
> Audience: contributor humano o agente IA que entra al repo por
> primera vez. Diseñado para responder "¿qué hago ahora?" en < 5 min.

## Tabla de contenidos

1. [Quickstart (TL;DR)](#1-quickstart-tldr)
2. [El modelo de trabajo en 8 pasos](#2-el-modelo-de-trabajo-en-8-pasos)
3. [SOT (Source of Truth) — dónde vive cada cosa](#3-sot-source-of-truth--dónde-vive-cada-cosa)
4. [Cómo crear una EP (workflow completo)](#4-cómo-crear-una-ep-workflow-completo)
5. [Cómo cerrar una EP](#5-cómo-cerrar-una-ep)
6. [Cómo mergear un PR](#6-cómo-mergear-un-pr)
7. [Cómo hacer un release](#7-cómo-hacer-un-release)
8. [Quality gates — qué debe pasar antes de cada transición](#8-quality-gates--qué-debe-pasar-antes-de-cada-transición)
9. [Escalación — qué hacer cuando algo se traba](#9-escalación--qué-hacer-cuando-algo-se-traba)
10. [Escalabilidad — cómo sumar contribuidores sin romper el SSD](#10-escalabilidad--cómo-sumar-contribuidores-sin-romper-el-ssd)
11. [Roles y permisos](#11-roles-y-permisos)
12. [Anti-patterns del SSD](#12-anti-patterns-del-ssd)
13. [Índice de metodologías (deep dives)](#13-índice-de-metodologías-deep-dives)
14. [Apéndice — entry points del workspace](#14-apéndice--entry-points-del-workspace)

---

## 1. Quickstart (TL;DR)

```bash
# 1. Clonar el workspace
git clone --recursive git@github.com-personal:Andresfernandezp94/neurox.git
cd neurox

# 2. Ver dónde estás
cat .sdd/STATE.md                         # épicas activas + próxima libre
ls .sdd/methodologies/                     # 7 metodologías operativas

# 3. Elegir repo y crear branch
git -C <submodule> switch -c fix/EP-0007-something

# 4. Hacer cambios siguiendo las metodologías
# (ver [sección 2](#2-el-modelo-de-trabajo-en-8-pasos) abajo)

# 5. Antes de commitear
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 6. Commit con Conventional Commits
git commit -m "fix(daemon): handle X gracefully

Refs: EP-0007"

# 7. Push y abrir PR
git push -u origin fix/EP-0007-something
gh pr create --base main --head fix/EP-0007-something
```

Eso es todo. Para detalles, seguí leyendo.

---

## 2. El modelo de trabajo en 8 pasos

Este es el **único workflow canónico** del workspace. Aplica a **todo PR**
(grande o chico, humano o agente, feature o fix).

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ 1.LEER   │───▶│2.PLANEAR │───▶│ 3.IDEAR  │───▶│4.BRANCH  │
│   SSD    │    │  alcance │    │  el diff │    │  + work  │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
                                                     │
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ 8.MERGE  │◀───│ 7.REVIEW │◀───│ 6.PR     │◀───│5.VERIFY  │
│  + borro │    │  + fix   │    │  + CI    │    │  local   │
│  branch  │    │          │    │          │    │          │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
```

### Paso 1: LEER el SSD

**Regla dura**: nunca toco código sin antes leer el SSD del repo y del entry point.

```bash
# Ver el estado actual del workspace
cat .sdd/STATE.md

# Ver la constitución (qué principios G1..G11 aplican)
cat .sdd/CONSTITUTION.md

# Ver la glosario (vocabulario canónico)
cat .sdd/GLOSSARY.md

# Ver las convenciones de prefijos y estados
cat .sdd/INDEXING.md

# Ver la estructura jerárquica del SOT
cat .sdd/GOV-STRUCTURE.md

# Si voy a trabajar en una EP, leer su proposal.md + design.md + tasks.md
cat .sdd/archived/EP-0004-*/proposal.md
```

Si la tarea **no encaja** en ninguna EP existente → **paso 2**.

### Paso 2: PLANEAR el alcance

Tres decisiones críticas antes de tocar código:

| Pregunta | Si SÍ | Si NO |
|---|---|---|
| ¿Esto es > 1 día de trabajo? | Crear una EP (paso 4) | Commit directo |
| ¿Esto cruza repos? | EP multi-repo (specs/EP-NNNN-NN-`<repo>`) | PR en 1 repo |
| ¿Esto cambia contratos HTTP/WS? | ADR (Architecture Decision Record) | Sin ADR |

**Regla**: si dudás entre EP y commit, **creá una EP**. Las EPs son
baratas; los commits sin contexto son caros de mantener.

### Paso 3: IDEAR el diff (mental o en docs)

Antes de escribir código, **decidí el diff objetivo**. La pregunta
que responder:

> "¿Qué archivos cambian, qué se agrega, qué se borra, qué se mueve?"

Si la respuesta no entra en 1 pantalla, **dividir en commits más
chicos** (cada uno mergeable independientemente).

### Paso 4: BRANCH + work

```bash
# Prefijo semántico según Conventional Commits
git switch -c feat/<slug>
git switch -c fix/<slug>
git switch -c refactor/<slug>
git switch -c docs/<slug>
git switch -c chore/<slug>
git switch -c test/<slug>

# Reglas del nombre (method-06):
# - kebab-case, lowercase
# - max 60 chars
# - sin tildes, sin caracteres especiales
# - sin prefijos redundantes (NO feat-feat-x)
```

### Paso 5: VERIFY local

**Regla dura**: nada sale de tu máquina sin pasar este checklist.

```bash
# 1. Format
cargo fmt --all

# 2. Lint (zero warnings, zero errors)
cargo clippy --workspace --all-targets -- -D warnings
pnpm exec eslint . --ext .ts,.tsx      # si el repo tiene TS

# 3. Tests
cargo test --workspace --exclude <repo-que-no-compila>
pnpm test                              # si el repo tiene TS

# 4. Build
cargo build --release
pnpm build
```

Si **cualquier paso falla**, **no commitear**. Fix forward, no `git
commit --amend` para tapar algo.

### Paso 6: PR + CI

```bash
git push -u origin <branch>

# Body del PR usa la plantilla (ver method-04-workflow-prs.md)
gh pr create --base main --head <branch> \
  --title "fix(daemon): handle X gracefully" \
  --body "## What
...

## Why
...

## Tests
- [ ] <test 1>
- [ ] <test 2>

Refs: EP-0007"
```

CI corre **automáticamente** después del push. Si CI falla, **fix
forward** — no commit de "fix CI" (squash en local antes de pushear).

### Paso 7: REVIEW + fix

- **Esperar 24h** (ideal 4h si no es grande)
- **Address cada comentario** explícitamente (resolve thread)
- **`[blocking]` vs `[nit]`**: solo blocking bloquea el merge
- **Self-review antes** de pedir review (leer el diff como reviewer)

### Paso 8: MERGE + cleanup

- **Merge strategy**: `squash and merge` (default) o `rebase and merge`
  (si los commits tienen valor histórico individual)
- **Borrar branch** después de merge (local + remote)
- **Volver a main** y `git pull --rebase`

---

## 3. SOT (Source of Truth) — dónde vive cada cosa

El SSD (Source of Truth) está en `.sdd/` del workspace y **de cada
sub-repo**. La regla **dura**: **una sola fuente de verdad por
concepto**.

| Si necesitás documentar... | Va en... | Naming |
|---|---|---|
| Principios inmutables del workspace | [`.sdd/CONSTITUTION.md`](./CONSTITUTION.md) | UPPERCASE |
| Vocabulario canónico | [`.sdd/GLOSSARY.md`](./GLOSSARY.md) | UPPERCASE |
| Convenciones de prefijos/estados | [`.sdd/INDEXING.md`](./INDEXING.md) | UPPERCASE |
| Estructura jerárquica del SOT | [`.sdd/GOV-STRUCTURE.md`](./GOV-STRUCTURE.md) | UPPERCASE |
| Cómo trabajar día a día | este doc ([`.sdd/GOVERNANCE.md`](./GOVERNANCE.md)) | UPPERCASE |
| Path convention `@/` | [`.sdd/CONVENTION.md`](./CONVENTION.md) | UPPERCASE |
| Estado actual de EPs | [`.sdd/STATE.md`](./STATE.md) | UPPERCASE |
| Épica abierta (en propuesta) | `.sdd/changes/EP-NNNN-<slug>/` | numeric |
| Épica cerrada (inmutable) | `.sdd/archived/EP-NNNN-<slug>/` | numeric |
| Índice de EPs cerradas | [`.sdd/archived/REGISTRO.md`](./archived/REGISTRO.md) | UPPERCASE |
| Decisión arquitectural | `.sdd/decisions/ADR-NNNN-<slug>.md` | UPPERCASE |
| Workflow operativo (7 guías) | `.sdd/methodologies/method-NN-<slug>.md` | lowercase |
| Plantilla de EP | `.sdd/templates/ep-template/` | lowercase |

**Regla**: si un doc está en 2+ lugares, **es un bug** — borrá la copia y
linkeá al original.

---

## 4. Cómo crear una EP (workflow completo)

**Trigger**: trabajo > 1 día, o cruza repos, o cambia contrato.

```bash
# 1. Ver próximo número libre
cat .sdd/STATE.md | grep "Última"

# 2. Crear la EP (numeración auto)
bash .sdd/bin/start-epic.sh mi-slug-descriptivo

# 3. Llenar proposal.md (qué + por qué + criterios)
$EDITOR .sdd/changes/EP-NNNN-mi-slug/proposal.md

# 4. Si toca varios repos, agregar specs
mkdir -p .sdd/changes/EP-NNNN-mi-slug/specs/EP-NNNN-01-daemon
$EDITOR .sdd/changes/EP-NNNN-mi-slug/specs/EP-NNNN-01-daemon/requirements.md

# 5. Llenar design.md (decisiones + contratos)
$EDITOR .sdd/changes/EP-NNNN-mi-slug/design.md

# 6. Llenar tasks.md (plan por fases con steps)
$EDITOR .sdd/changes/EP-NNNN-mi-slug/tasks.md

# 7. Commit del SSD
git add .sdd/changes/EP-NNNN-mi-slug/
git commit -m "docs(sdd): open EP-NNNN mi-slug-descriptivo"
```

Frontmatter obligatorio en proposal.md (ver [method-07](./methodologies/method-07-estados-y-steps.md)):

```markdown
# EP-NNNN — Título descriptivo

> **ID**: EP-NNNN
> **Type**: refactor | feat | fix | chore
> **Status**: draft:proposal
> **Created**: YYYY-MM-DD
> **Updated**: YYYY-MM-DD
> **Owner**: @<github-handle>
> **Reviewers**: [@<handle1>, @<handle2>]

## Problema
[1-3 párrafos]

## Solución propuesta
[Alto nivel]

## Criterios de aceptación
- [ ] Criterio 1
- [ ] Criterio 2

## Riesgos
[Opcional]
```

**Regla**: el **Status** va como `estado:sub-estado` (ver [method-07](./methodologies/method-07-estados-y-steps.md#sub-estados-granularidad-dentro-de-un-estado)).

---

## 5. Cómo cerrar una EP

**Trigger**: el `Status` llegó a `done:verified` (implementada, deployada, validada manualmente).

```bash
# 1. Correr close-epic.sh (atómico)
bash .sdd/bin/close-epic.sh EP-NNNN

# Qué hace el script (debería ser atómico — ver EP-0007 que lo implementa):
# - Mueve changes/EP-NNNN/ → archived/EP-NNNN/
# - Actualiza REGISTRO.md
# - Actualiza STATE.md (saca de activas, actualiza última numerada)
# - Deja REGISTRO.md con la fila nueva
# Exit no-cero si algo falla
```

**Pre-condición**: la EP tiene el `Status: done:merged` (o `done:verified` si es sin deploy) en **todos** sus docs (proposal.md, design.md, tasks.md, specs/*/requirements.md).

---

## 6. Cómo mergear un PR

Ver [`method-04-workflow-prs.md`](./methodologies/method-04-workflow-prs.md) para el detalle. **TL;DR**:

```bash
# 1. Verificar CI + approvals + no conflicts
gh pr checks
gh pr view --json reviewDecision

# 2. Merge (squash and merge es el default)
gh pr merge --squash --delete-branch
```

**Regla**: solo `squash and merge` o `rebase and merge`. **NUNCA** regular
merge (genera merge commits que ensucian `main`).

---

## 7. Cómo hacer un release

Ver [`method-05-checklist-release.md`](./methodologies/method-05-checklist-release.md) para el detalle completo. **TL;DR**:

```bash
# Pre-release (T-1 día)
cargo test --workspace              # tests
cargo build --release              # build
cargo clippy --all-targets -- -D warnings  # lint
# Actualizar CHANGELOG.md, README.md, .sdd/archived/REGISTRO.md

# Release (T-0)
VERSION="0.X.Y"
git tag -a "v${VERSION}" -m "Release v${VERSION}"
git push origin "v${VERSION}"
gh release create "v${VERSION}" --notes-file RELEASE_NOTES.md

# Post-release (T+1)
# Verificar binarios en CI
# Anunciar si aplica
```

---

## 8. Quality gates — qué debe pasar antes de cada transición

| Transición | Gate | Comando |
|---|---|---|
| `draft:*` → `proposed:*` | proposal.md + design.md + tasks.md completos | (manual review) |
| `in-progress:*` → `in-review:*` | Lint pasa | `cargo clippy --all-targets -- -D warnings` |
| `in-progress:*` → `in-review:*` | Tests pasan | `cargo test --workspace` |
| `in-progress:*` → `in-review:*` | Build pasa | `cargo build --release` |
| `in-progress:*` → `in-review:*` | Format pasa | `cargo fmt --all -- --check` |
| `in-progress:*` → `in-review:*` | Sin TODOs sin ticket | `rg 'TODO\([^)]+\)' --type-add` |
| `in-progress:*` → `in-review:*` | Sin CJK en código | `rg '[一-鿿]'` |
| `in-progress:*` → `in-review:*` | Sin secrets hardcoded | `rg -i 'api[_-]?key\|password\|token' src/` |
| `in-review:*` → `merged` | CI pasa en PR | (GitHub Actions) |
| `in-review:*` → `merged` | ≥1 approval (2 si breaking) | (GitHub PR review) |
| `in-review:*` → `merged` | Sin `[blocking]` sin resolver | (GitHub PR review) |
| `merged` → `done:deployed` | Manual smoke test | (manual) |
| `done:deployed` → `closed:archived` | `close-epic.sh EP-NNNN` exit 0 | `bash .sdd/bin/close-epic.sh EP-NNNN` |

**Regla dura**: si CUALQUIER gate falla, **no** avanzar al siguiente
estado. Fix forward.

---

## 9. Escalación — qué hacer cuando algo se traba

| Situación | Acción |
|---|---|
| PR abierto > 3 días sin review | Ping al reviewer; si sigue, pingar a otro |
| Conflicto técnico entre contributors | Abrir ADR, no resolver en comentarios |
| EP bloqueada por issue de otro repo | Documentar en frontmatter como `**Blocked-by**: #<n>` |
| Gate falla repetidamente (> 3 veces) | Pausar la EP (`Status: paused:<razón>`) y abrir issue de soporte |
| Convención ambigua (2 formas válidas) | Abrir PR a [method-NN-*.md](./methodologies/) documentando la regla |
| Bug crítico en producción | Rollback según [method-05-checklist-release.md](./methodologies/method-05-checklist-release.md#rollback-si-algo-sale-mal) |
| Security issue | Ver [.sdd/security/process.md](./security/process.md) (no commitear nada hasta disclosure) |

---

## 10. Escalabilidad — cómo sumar contribuidores sin romper el SSD

### Onboarding de un nuevo contributor (humano o agente IA)

```
Día 1: leer GOVERNANCE.md (este doc, 20 min)
       ↓
Día 1: leer CONSTITUTION.md + GLOSSARY.md (15 min)
       ↓
Día 2: leer INDEXING.md + GOV-STRUCTURE.md (15 min)
       ↓
Día 2: skimmear las 7 metodologías (30 min)
       ↓
Día 3: hacer un fix chico end-to-end (PR #1)
       ↓
Día 3: code review de alguien con experiencia
```

**Regla**: nadie toca código sin haber pasado los pasos 1-3.

### Multi-contributor — cómo evitar colisiones

| Riesgo | Mitigación |
|---|---|
| 2 personas editan la misma EP | Asignar Owner único en frontmatter |
| 2 personas abren EPs con el mismo número | `bin/start-epic.sh` calcula el siguiente número; nunca elegir a mano |
| 2 personas modifican el mismo file en REGISTRO | `bin/close-epic.sh` es atómico; no editar a mano |
| Convención ambigua en equipo | Llevar a `method-NN-*.md` con PR, no comentar en Slack |

### Cómo el SOT escala a N repos

Cada repo del workspace tiene su **propio** `.sdd/` (local) con
épicas específicas, **más** el `.sdd/` raíz (este) con governance
global. Convención:

- **Reglas globales** (CONSTITUTION, GLOSSARY, GOVERNANCE) → **raíz**
- **Reglas locales** (épicas, decisiones) → **repo local**
- **Conflict**: gana la raíz (CONSTITUTION > repo local)

Para N > 10 repos, considerar `.sdd/bin/sync.sh` que propague
cambios governance a todos los sub-repos vía git subtree o
submodule pointer bumps.

---

## 11. Roles y permisos

### Por capacidad

| Rol | Puede | No puede |
|---|---|---|
| **Owner** (de una EP) | abrir, mergear, cerrar la EP | ignorar conventions deCONSTITUTION |
| **Reviewer** | aprobar, comentar, bloquear | mergear sin aprobación de otro reviewer |
| **Contributor** (cualquier PR) | abrir PR, comentar, mergear después de review | mergear su propio PR sin review |
| **Bot/agente IA** | abrir PRs, comentar, ejecutar `bin/*` | mergear PRs (human-in-the-loop) |

### Por responsabilidad en el SSD

| Rol | Edita |
|---|---|
| **SOT maintainer** (@andres_fernandez por ahora) | `CONSTITUTION.md`, `GOVERNANCE.md`, `INDEXING.md`, `GOV-STRUCTURE.md` |
| **Repo maintainer** | `STATE.md`, `REGISTRO.md`, `decisions/`, `methodologies/` del repo |
| **Cualquier contributor** | `proposal.md`, `design.md`, `tasks.md`, `specs/` de su propia EP |

---

## 12. Anti-patterns del SSD

- ❌ **Crear archivos `.md` sin referenciarlos** desde GOVERNANCE/INDEXING
- ❌ **Duplicar info** en 2+ lugares del SSD (linkeá, no copies)
- ❌ **Editar CONSTITUTION** sin abrir ADR que documente el cambio
- ❌ **Saltar el SOT** y commitear "para probar en main" (no va a main sin SSD)
- ❌ **Crear EPs sin Status** en frontmatter (machine-readable es obligatorio)
- ❌ **Números de EP no correlativos** (la regex del `start-epic.sh` los rechaza)
- ❌ **Mezclar status**: `Status: in-progress:implementation | done:verified` (imposible)
- ❌ **Force-push a main** (rompe historial para todos — **NUNCA**)
- ❌ **Cerrar una EP sin `close-epic.sh`** (rompe consistencia de REGISTRO/STATE)
- ❌ **Borrar/modificar el SOT sin dejar un rastro** (todo cambio queda en `git log`)

---

## 13. Índice de metodologías (deep dives)

| # | Doc | Para qué | Cuándo leerlo |
|---|-----|----------|--------------|
| 01 | [method-01-flujo-de-trabajo.md](./methodologies/method-01-flujo-de-trabajo.md) | Workflow diario en 8 pasos | Al arrancar el día |
| 02 | [method-02-buenas-practicas-documentacion.md](./methodologies/method-02-buenas-practicas-documentacion.md) | Cómo escribir docs que no se desactualicen | Al escribir o actualizar un doc |
| 03 | [method-03-buenas-practicas-codigo.md](./methodologies/method-03-buenas-practicas-codigo.md) | Reglas de naming, errores, comentarios | Al escribir código |
| 04 | [method-04-workflow-prs.md](./methodologies/method-04-workflow-prs.md) | Cómo abrir, revisar y mergear PRs | Al abrir/cerrar un PR |
| 05 | [method-05-checklist-release.md](./methodologies/method-05-checklist-release.md) | Pasos para taggear y publicar versión | Al hacer release |
| 06 | [method-06-convenciones-git.md](./methodologies/method-06-convenciones-git.md) | Branches, commits, submodules | Al trabajar con git |
| 07 | [method-07-estados-y-steps.md](./methodologies/method-07-estados-y-steps.md) | Máquina de estados del SOT | Al crear/actualizar una EP |
| 08 | [method-08-doc-update-mandatory.md](./methodologies/method-08-doc-update-mandatory.md) | Workflow de propagación código → doc (G12) | Al abrir cualquier PR |

> **Regla**: este GOVERNANCE.md es la **puerta de entrada**. Los
> method-*.md son los **detalles profundos**. Si tenés que decidir algo y
> este doc no lo cubre, **andá al method-*.md relevante antes de
> tomar la decisión**.

---

## 14. Apéndice — entry points del workspace

| Repo | `.sdd/` | Propósito |
|---|---|---|
| `neurox` (raíz) | ✅ este directorio | Governance global, EPs cross-repo |
| `daemon` | sub-repo `.sdd/` (symlinks) | El daemon en sí |

### Binarios del SSD (raíz)

```bash
# SOT management
bin/start-epic.sh <slug>            # crea EP-NNNN-<slug>/ en changes/
bin/close-epic.sh EP-NNNN            # mueve a archived/ + actualiza REGISTRO/STATE
bin/next-step.sh                    # sugiere próximo paso
bin/list-status.sh                  # reporte de todas las EPs
bin/summary.sh                      # resumen del SOT
```

> ⚠️ **Nota**: `close-epic.sh` actualmente imprime un "recordatorio de
> regenerar STATE.md" en vez de actualizarlo atómicamente. **EP-0007**
> (SOT governance hardening) está abierta para arreglar esto. Hasta
> entonces, **siempre editar STATE.md a mano después de correr
> close-epic.sh**.

### Cómo navegar el SOT eficientemente

```bash
# Estado de las EPs
rg "^\> \*\*Status\*\*:" .sdd/ | sort | uniq -c

# EPs en progreso
rg -l "Status.*in-progress" .sdd/

# Fixes abiertos
rg -l "Status.*open" .sdd/

# Referencias a un file
rg "<archivo>" .sdd/

# Cross-refs a un EP específico
rg "EP-0004" .sdd/
```

---

## Version

- **1.0** (2026-08-29) — inicial
  - 14 secciones cubriendo workflow, SOT, escalabilidad, anti-patterns
  - 7 metodologías linkeadas como deep dives
  - 10 quality gates documentados
  - 12 anti-patterns del SSD
  - 6 binarios del SSD referenciados

---

**Convención v1.3**: este doc es UPPERCASE (`GOVERNANCE.md`).
Cada nuevo governance doc del SOT root debe seguir la misma convención
(véase `INDEXING.md` v1.3 changelog).