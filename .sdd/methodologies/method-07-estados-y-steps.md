# Máquina de Estados de Épicas y Features

> **Sistema de tracking** del ciclo de vida de épicas, features y fixes. Cada documento del SOT puede tener un **estado** y **sub-estado**, con **steps explícitos** que permiten tracking granular.
>
> Ver también: [INDEXING.md](../INDEXING.md) para prefijos del SOT y [method-01-flujo-de-trabajo.md](./method-01-flujo-de-trabajo.md) para el workflow general.

## Por qué un sistema de estados

Un SOT sin estados es una lista plana de documentos. Un SOT **con estados** permite:

- **Tracking granular**: saber exactamente en qué fase está cada trabajo
- **Búsquedas eficientes**: "todas las épicas in-review", "todos los fixes blocked"
- **Reportes de progreso**: contar cuántas épicas hay en cada estado
- **Identificar bloqueos**: ver rápidamente qué está parado y por qué
- **Trabajo agentico**: los agentes saben qué hacer según el estado

## Estados principales (top-level)

| Estado | Significado | Siguiente paso típico |
|--------|-------------|----------------------|
| `draft` | Recién creado, en construcción | → `proposed` cuando se completa proposal.md |
| `proposed` | Proposal listo, esperando review/validación | → `approved` o → `rejected` |
| `approved` | Aprobada para implementar | → `in-progress` cuando alguien la toma |
| `in-progress` | Alguien la está implementando activamente | → `in-review` cuando hay un PR abierto |
| `in-review` | PR abierto, esperando code review | → `merged` o → `changes-requested` |
| `changes-requested` | Review pidió cambios | → `in-progress` (volver a implementar) |
| `merged` | Mergeada a main | → `done` después de deploy/verificación |
| `done` | Implementada, deployada, verificada | → `closed` (archivar) |
| `closed` | Archivada en `.sdd/archived/` (inmutable) | (estado final) |
| `blocked` | Bloqueada por algo externo | → vuelve al estado anterior cuando se desbloquea |
| `paused` | Pausada temporalmente (ej. esperando feature) | → vuelve al estado anterior cuando se reanuda |
| `rejected` | Proposal rechazada (no se va a implementar) | → `closed` (archivar con nota) |
| `superseded` | Reemplazada por otra épica más nueva | → `closed` |

## Sub-estados (granularidad dentro de un estado)

Los **sub-estados** se escriben como `estado:subestado`:

| Sub-estado | Cuándo aplica | Steps típicos |
|------------|--------------|---------------|
| `draft:proposal` | Solo existe el proposal.md | [ ] Definir problema y visión |
| `draft:design` | Proposal + design en construcción | [ ] Diseñar contratos, [ ] Decisiones técnicas |
| `draft:tasks` | Falta el tasks.md | [ ] Plan por fases, [ ] Estimación de horas |
| `proposed:review-pending` | Proposal completo, esperando feedback | [ ] Asignar reviewer, [ ] Recolectar comentarios |
| `proposed:specs-pending` | Aprobada, faltan specs | [ ] Una spec por repo, [ ] Criterios de aceptación |
| `approved:ready-to-start` | Todo listo, falta tomar la épica | [ ] Asignar owner, [ ] Crear branch |
| `in-progress:scaffolding` | Empezando, creando estructura | [ ] Branches, [ ] Estructura de archivos |
| `in-progress:implementation` | Código en desarrollo | [ ] Core logic, [ ] Edge cases, [ ] Tests |
| `in-progress:tests` | Código listo, faltan tests | [ ] Unit, [ ] Integration, [ ] Coverage |
| `in-progress:docs` | Todo listo, faltan docs | [ ] CHANGELOG, [ ] README, [ ] SSD updates |
| `in-review:pr-open` | PR abierto | [ ] CI passing, [ ] Reviewers asignados |
| `in-review:feedback-addressed` | Comentarios del review aplicados | [ ] Re-review requested, [ ] Tests actualizados |
| `in-review:approved` | Aprobada, esperando merge | [ ] Squash merge, [ ] Borrar branch |
| `in-progress:merge` | En proceso de merge | [ ] CI passing en main, [ ] Conflict resolution |
| `done:deployed` | Deployada a producción | [ ] Bundle actualizado, [ ] Cache busted |
| `done:verified` | Verificada manualmente en prod | [ ] Smoke tests, [ ] Sin errores en logs |
| `closed:archived` | Archivado en `.sdd/archived/` | (estado final) |
| `closed:superseded` | Reemplazada por otra | [ ] Link al sucesor en frontmatter |
| `closed:rejected` | No se va a implementar | [ ] Razón en frontmatter |
| `blocked:external` | Bloqueada por issue de otro repo | [ ] Link al blocker en frontmatter |
| `blocked:review` | Bloqueada por decisión arquitectural pendiente | [ ] Link al ADR en draft |
| `paused:waiting-feature` | Esperando otra feature | [ ] Link a la feature |

## Formato en frontmatter

El estado va en el frontmatter del documento:

```markdown
# <Título>

> **ID**: <tipo>-<número>
> **Status**: <estado>:<sub-estado>   # ej: "in-progress:implementation"
> **Created**: YYYY-MM-DD
> **Updated**: YYYY-MM-DD
> **Owner**: <nombre>
> **Reviewers**: [@user1, @user2]
> **Related**: [link a otros docs]

<contenido>
```

## Steps (checklist dentro de cada sub-estado)

Cada sub-estado tiene un **checklist de steps** que se completan secuencialmente. Formato:

```markdown
## Steps

### in-progress:implementation
- [ ] Crear branch `feat/EP-NNNN-<slug>`
- [ ] Scaffolding: estructura de archivos
- [ ] Core logic en `src/<module>.rs`
- [ ] Tests unitarios en `src/<module>_test.rs` o `tests/`
- [ ] Edge cases cubiertos
- [ ] Sin warnings de `cargo clippy --all-targets`

### in-progress:tests
- [ ] Unit tests pasan
- [ ] Integration tests pasan
- [ ] Coverage >= 70% para código nuevo

### in-progress:docs
- [ ] CHANGELOG.md actualizado
- [ ] README.md actualizado
- [ ] .sdd/INDEX.md actualizado
- [ ] ADR creado si hay decisiones arquitecturales

### in-review:pr-open
- [ ] CI pasa (fmt, clippy, test, build)
- [ ] Al menos 1 aprobación
- [ ] Sin comentarios `[blocking]` sin resolver
```

## Workflow típico (ejemplo de feature)

```markdown
# EP-0016 — LLM Sampling Config

> **Status**: in-progress:implementation
> **Created**: 2026-08-08
> **Owner**: @andres
> **Reviewers**: []

## Steps

### draft:proposal ✅
- [x] Definir problema
- [x] Proponer solución

### draft:design ✅
- [x] Decisiones técnicas
- [x] Contratos

### draft:tasks ✅
- [x] Plan por fases
- [x] Estimación

### proposed:review-pending ✅
- [x] Self-review del proposal
- [x] PR con proposal abierto

### proposed:specs-pending 🔄
- [x] Spec 01 — backend changes
- [ ] Spec 02 — admin UI

### in-progress:implementation 🔄
- [x] Branch `feat/EP-0016-sampling` creada
- [ ] Backend: `LlmProviderConfig` extendido
- [ ] Backend: parsing de nuevos campos
- [ ] Admin: `ProviderForm` con sliders

### in-progress:tests ⏳
- [ ] Unit tests
- [ ] Integration tests

### in-review:pr-open ⏳
- [ ] PR abierto
- [ ] CI passing
- [ ] Reviewers aprobados

### done:deployed ⏳
- [ ] Bundle deployado
- [ ] Verificado en prod
```

## Búsquedas por estado

```bash
# Todas las épicas en progreso
rg -l "Status.*in-progress" .sdd/

# Todos los fixes abiertos
rg -l "Status.*open" .sdd/fixes/

# Todas las épicas bloqueadas
rg -l "Status.*blocked" .sdd/

# Features esperando review
rg -l "Status.*in-review" .sdd/

# Sub-estados específicos
rg "Status.*in-progress:tests" .sdd/

# Features del agente X
rg -l "Owner.*andres" .sdd/
```

## Reportes de progreso

```bash
# Contar por estado
rg "^\> \*\*Status\*\*:" .sdd/ | sort | uniq -c | sort -rn

# Output esperado:
#    12 in-progress:implementation
#     5 in-review:pr-open
#     3 done:deployed
#     1 blocked:external
```

## Reglas de transición

| Desde | Hacia | Trigger |
|-------|-------|----------|
| `draft:*` | `proposed:review-pending` | proposal.md + design.md + tasks.md completos |
| `proposed:*` | `approved:ready-to-start` | PR de propuesta mergeado |
| `approved:*` | `in-progress:scaffolding` | branch creada, primer commit |
| `in-progress:scaffolding` | `in-progress:implementation` | estructura básica lista |
| `in-progress:implementation` | `in-progress:tests` | código compila, sin warnings |
| `in-progress:tests` | `in-review:pr-open` | PR abierto, CI passing |
| `in-review:pr-open` | `in-review:feedback-addressed` | comentarios del review aplicados |
| `in-review:*` | `in-progress:merge` | PR aprobado |
| `in-progress:merge` | `done:deployed` | mergeado + deployed |
| `done:deployed` | `done:verified` | verificado en prod |
| `done:verified` | `closed:archived` | `bash scripts/close-epic.sh EP-NNNN` |
| cualquier | `blocked:external` | algo externo bloquea |
| cualquier | `paused:waiting-feature` | esperando otra feature |
| `blocked:*` | estado anterior | cuando se desbloquea |

## Frontmatter extendido

Para features complejas, el frontmatter puede incluir:

```markdown
# <Título>

> **ID**: <tipo>-<número>
> **Status**: <estado>:<sub-estado>
> **Created**: YYYY-MM-DD
> **Updated**: YYYY-MM-DD
> **Owner**: <nombre>
> **Reviewers**: [@user1, @user2]
> **Branch**: feat/<slug>
> **PR**: #<número>
> **Related**:
>   - [EP-NNNN — título](../epicas/EP-NNNN-titulo.md)
>   - [fix-NNN — título](../fixes/fix-NNN-slug.md)
>   - [Issue #123](https://github.com/...)
> **Blocks**: [Issue #456](https://github.com/...)  # opcional, si bloquea otra cosa
> **Blocked-by**: [Issue #789](https://github.com/...)  # opcional, si está bloqueada

<contenido>

## Steps

### <sub-estado actual>
- [ ] Step 1
- [ ] Step 2
- [x] Step 3 (completado)

### <siguiente sub-estado>
- [ ] Step 4
- [ ] Step 5
```

## Anti-patterns

- ❌ **Estado sin sub-estado**: usar `in-progress` en vez de `in-progress:implementation` cuando ya sepas la fase
- ❌ **Sub-estado sin steps**: cada sub-estado debe tener su checklist
- ❌ **Steps sin completar**: dejar checks viejos como `- [x]` sin documentar cuándo
- ❌ **Estados inconsistentes**: usar "done" en un lugar y "completed" en otro
- ❌ **No actualizar el estado**: dejar `draft:proposal` cuando ya está en `in-progress:implementation`
- ❌ **Mezclar estados**: `Status: in-progress:implementation | done:verified` (imposible)

## Version

- **1.0** (2026-08-08) — inicial
  - 13 estados top-level
  - 20 sub-estados
  - Reglas de transición documentadas
  - Búsquedas eficientes con `rg`