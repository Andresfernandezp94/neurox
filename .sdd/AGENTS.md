# AGENTS.md — Entry point for AI agents

> **Flujo de 5 pasos** para un agente IA (humano o bot) que entra al
> repo por primera vez. Diseñado para responder "¿qué hago ahora?" en
> < 5 minutos.
>
> Versión: 1.0 (2026-08-29) | Status: active

> **REGLA CRÍTICA**: **toda la documentación vive en `.sdd/`**. Si
> necesitás buscar docs en otro sitio (sub-repos, meta-repo root, código),
> **volvé a `.sdd/`**. Los governance docs raíz son UPPERCASE; los EP
> templates y docs de operación son lowercase.

## Paso 1 — Leer el README

```bash
cat .sdd/README.md
```

El README es la **puerta de entrada al SOT**. Tiene una tabla
"¿Qué necesitás?" con los comandos exactos para cada necesidad.

## Paso 2 — Leer el GOVERNANCE

```bash
cat .sdd/GOVERNANCE.md
```

El GOVERNANCE es el **handbook único** de cómo trabajar. Tiene 14
secciones cubriendo: workflow, SOT, EPs, quality gates, escalación,
escalabilidad, roles, anti-patterns, y las 7 metodologías como
deep dives.

## Paso 3 — Ver el estado actual

```bash
cat .sdd/STATE.md
```

STATE.md es **machine-readable**: dice qué EPs están activas, cuál
es la última numerada, y cuál es la próxima libre. Si hay una
EP activa que match tu tarea, **continuá con esa** (no crees una
nueva).

## Paso 4 — Ver el resumen

```bash
bash .sdd/bin/summary.sh
```

El summary script da una vista de 1 pantalla del workspace: branch,
modified files, submodules, y estado del SOT.

## Paso 5 — Validar consistencia

```bash
bash .sdd/bin/lint.sh
```

El lint script verifica que el SOT sea consistente (frontmatter,
refs, numeración, naming, etc.). Si exit 0, todo OK. Si exit 1,
lista los issues con archivo:línea.

## Si necesitás crear una EP

```bash
bash .sdd/bin/start-epic.sh <slug-kebab-case>
# Edita .sdd/changes/EP-NNNN-<slug>/{proposal,design,tasks}.md
# Opcional: crea .sdd/changes/EP-NNNN-<slug>/specs/EP-NNNN-01-<repo>/requirements.md
git add .sdd/changes/EP-NNNN-*/
git commit -m "docs(ssd): open EP-NNNN <título>"
```

Más detalles: ver `.sdd/GOVERNANCE.md` §4 (Cómo crear una EP).

## Si necesitás cerrar una EP

```bash
bash .sdd/bin/close-epic.sh EP-NNNN
```

El script es **atómico** desde la EP-0007 R-004: mueve el folder,
actualiza REGISTRO.md Y STATE.md en 1 solo paso.

## Si tenés que mergear un PR

Ver `.sdd/methodologies/method-04-workflow-prs.md` para el detalle
completo. TL;DR:
- CI pasa (lint + tests + build)
- ≥1 approval (2 si breaking)
- Squash and merge (default)
- Borrar branch

## Si tenés que hacer un release

Ver `.sdd/methodologies/method-05-checklist-release.md`. TL;DR:
- T-1 día: actualizar CHANGELOG, validar tests, build release
- T-0: git tag -a vX.Y.Z + git push
- T+1: verificar binarios en CI

## Reglas duras (NO NEGOCIABLES)

1. **Toda doc governance en root → UPPERCASE** (CONSTITUTION, GOVERNANCE,
   GLOSSARY, INDEXING, etc.). Ver `.sdd/INDEXING.md` v1.3 changelog.
2. **Toda EP tiene `**Status**:` machine-readable** (formato
   `top:sub` per `.sdd/methodologies/method-07-estados-y-steps.md`).
3. **Toda numeración de EP es sequential numeric** (EP-0001, EP-0002, ...).
   Sin fecha, sin gaps. `bin/start-epic.sh` calcula el siguiente.
4. **Toda la doc vive en `.sdd/`**. Si buscás docs en otro sitio, **volvé
   a `.sdd/`**. Los sub-repos solo linkean.
5. **Toda PR pasa `bin/lint.sh` antes de mergear** (exit 0).

## Anti-patterns (NO HACER)

- ❌ Crear archivos `.md` en sub-repos o en el meta-repo root — todo va en `.sdd/`
- ❌ Usar formato fecha para EPs (EP-YYYY-MM-DD) — fue removido en v1.2
- ❌ Saltar el SOT y commitear "para probar en main" — no va a main sin SSD
- ❌ Editar CONSTITUTION o GOVERNANCE sin abrir ADR
- ❌ Force-push a main — rompe historial para todos
- ❌ Mezclar cambios no relacionados en un commit
- ❌ Crear un .sdd/ con naming en lowercase para governance (v1.3)

## Estructura del SOT (mapa rápido)

```
.sdd/
├── README.md                ← entry point: tabla "¿Qué necesitás?"
├── GOVERNANCE.md            ← handbook único (14 secciones)
├── CONSTITUTION.md          ← principios G1..G11
├── GLOSSARY.md              ← vocabulario canónico
├── INDEXING.md              ← prefijos + estados + búsquedas
├── STATE.md                 ← épicas activas + última numerada
├── GOV-STRUCTURE.md         ← mapa de jerarquías
├── AGENTS.md                ← este doc (entry point para agentes)
├── CONVENTION.md            ← path convention @/
├── changes/                 ← épicas abiertas
├── archived/                ← épicas cerradas + REGISTRO.md
├── decisions/               ← ADRs (inmutables)
├── methodologies/           ← 7 metodologías operativas
├── research/                ← notas framework-agnostic
├── security/                ← procesos de seguridad
├── templates/               ← plantillas de EP
└── bin/                     ← scripts de gestión (start/close/lint/summary)
```

## Cómo pedir ayuda

Antes de preguntar "cómo hago X":

1. Lee `.sdd/README.md` (tabla)
2. Lee `.sdd/GOVERNANCE.md` (sección relevante)
3. Si X es un workflow → busca en `.sdd/methodologies/method-NN-*.md`
4. Si X es naming/estructura → lee `.sdd/INDEXING.md`

Si después de esos 4 pasos no encontraste la respuesta, **preguntá**
(a un humano o abrí un ADR). El SOT no cubre todo — los gaps se
cierran con PRs a `.sdd/`.

## Tareas comunes (convención)

| Si querés... | Archivo o sección |
|---|---|
| Crear una EP | `.sdd/bin/start-epic.sh` + `GOVERNANCE.md` §4 |
| Cerrar una EP | `.sdd/bin/close-epic.sh` (atómico) |
| Ver el estado del SOT | `.sdd/STATE.md` + `.sdd/bin/summary.sh` |
| Verificar consistencia | `.sdd/bin/lint.sh` |
| Crear un ADR | `.sdd/decisions/adr-NNNN-<slug>.md` template |
| Ver convenciones de naming | `.sdd/INDEXING.md` |
| Ver el workflow diario | `.sdd/methodologies/method-01-flujo-de-trabajo.md` |
| Ver PR workflow | `.sdd/methodologies/method-04-workflow-prs.md` |
| Ver release process | `.sdd/methodologies/method-05-checklist-release.md` |
| Ver convenciones de git | `.sdd/methodologies/method-06-convenciones-git.md` |
| Ver máquina de estados | `.sdd/methodologies/method-07-estados-y-steps.md` |

## Onboarding de un nuevo contributor (humano o IA)

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

Regla: nadie toca código sin haber pasado los pasos 1-3.

## Convenciones v1.3 (resumen)

- **Governance docs** en `.sdd/` root → UPPERCASE
- **EP templates** (`proposal.md`, `design.md`, `tasks.md`,
  `requirements.md`) → lowercase
- **Status field** siempre presente, formato `top:sub` (e.g.,
  `in-progress:implementation`)
- **EP IDs** son sequential numeric (EP-0001, EP-0002, ...), sin fecha
- **ADRs** en `.sdd/decisions/ADR-NNNN-<slug>.md` (UPPERCASE, 4 dígitos)
- **methodologies** en `.sdd/methodologies/method-NN-<slug>.md` (lowercase, 2 dígitos)

Para detalles, ver `.sdd/INDEXING.md` v1.3 changelog.

## Version

- **1.0** (2026-08-29) — inicial (creado por EP-0007 R-008)
  - Flujo de 5 pasos para agentes IA
  - Tabla de entry points
  - Reglas duras + anti-patterns
  - Onboarding de nuevos contributors
  - Convenciones v1.3 resumidas