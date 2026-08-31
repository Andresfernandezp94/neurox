# HANDOVER — EP-0007 session state

> **Estado de la sesión del 2026-08-29** que trabajó la EP-0007
> `sot-governance-hardening`. Este doc existe para que cualquier agente
> (humano o IA) que abra el repo en una sesión nueva sepa exactamente
> dónde está y qué falta.
>
> **Identificador de la EP**: `EP-0007-sot-governance-hardening`
> **Rama**: `main`
> **Status en `STATE.md`**: `in-progress:implementation` (T-12 pendiente)
> **Próxima libre**: `EP-0008`

## TL;DR — qué falta

**T-12**: cerrar la EP-0007 atómicamente con `bin/close-epic.sh`.

3 sub-pasos:

1. **Arreglar 7 issues restantes** del `bin/lint.sh` (ver sección
   "Issues pendientes del lint" abajo). El script ya está commiteado
   con el fix de wildcards (T-10 + fix bash).
2. **Correr** `bash .sdd/bin/lint.sh` → debe pasar (exit 0).
3. **Correr** `bash .sdd/bin/close-epic.sh EP-0007` → la EP se archiva
   automáticamente (move + REGISTRO + STATE atómico).

Después de eso, `STATE.md` debe mostrar:

- "Última épica numerada: `EP-0007`"
- "Próxima libre: `EP-0008`"
- Tabla de "Épicas activas" vacía

## Tareas hechas (T-1..T-11) — todas commiteadas

| T-N | Commit | Descripción |
|---|---|---|
| Step 1 (start-epic) | `5f300a7` (parte) | `bash bin/start-epic.sh sot-governance-hardening` |
| Step 2 (proposal) | `5f300a7` (parte) | Llenar `proposal.md` con R-001..R-012 |
| Step 3 (tasks) | `5f300a7` (parte) | Llenar `tasks.md` con T-1..T-12 (sin "phases") |
| Step 4 (commit open) | (parte del setup) | Commit de apertura de la EP |
| T-1 | `51e7093` | Backport `**ID:**` + `**Type:**` a EP-0001/0002/0003 |
| T-2 | `6cba44b` | Fix `EP-0005` Legacy ID (era self-referential) |
| T-3 | `85d439b` | Renumerar refs en body de EP-0004/0005/0006 |
| T-4 + T-5 | `5f300a7` | `bin/close-epic.sh` atómico + fix bash arithmetic |
| T-6 | `c2ad2a3` (daemon) | `daemon/.sdd/` rename a UPPERCASE |
| T-7 | `862c036` (meta) | Bump submodule daemon |
| T-8 | `e326305` | Crear `AGENTS.md` (entry point 5-pasos para IA) |
| T-9 | `c4c9833` | `README.md` apunta a GOVERNANCE primero |
| T-10 | `9dcd387` | Crear `bin/lint.sh` con 9 checks |
| T-11 | `e28ff36` | `changes/.gitkeep` + borrar `template-epic-with-states.md` |
| (extra) | `f1922a3` | Frontmatter backport final + lint.sh wildcards fix |

## Commits de la sesión (12 total, en `main`)

```
11cff9b docs(meta-repo): add docs/README.md — index of module docs
9af60b4 docs(meta-repo): migrate agents/ → docs/agents/
e914f2c docs(meta-repo): migrate mcps/memory/docs/ → docs/mcp-memory/
03dde53 docs(meta-repo): migrate daemon/docs/ → docs/daemon/
f1922a3 docs(ssd): EP-0007 WIP — frontmatter backport + lint.sh wildcards fix
e28ff36 chore(ssd): changes/.gitkeep + remove duplicate template (R-011)
9dcd387 feat(ssd): add bin/lint.sh — SOT consistency validator
c4c9833 docs(ssd): README.md points to GOVERNANCE + AGENTS first
e326305 docs(ssd): add AGENTS.md — entry point for AI agents
862c036 chore(submodules): bump daemon with EP-0007 SOT naming migration
c2ad2a3 refactor(daemon): apply SOT v1.3 UPPERCASE to .sdd/
5f300a7 refactor(ssd): make close-epic.sh + start-epic.sh atomically arithmetic-safe
85d439b refactor(ssd): replace legacy EP-2026-08-* refs in body of EP-0004..0006
6cba44b docs(ssd): fix EP-0005 legacy id (was self-referential)
51e7093 docs(ssd): backport ID/Type frontmatter to EP-0001..EP-0003
```

## Migración de docs/ al meta-repo (paralela a la EP)

4 commits en el meta-repo que centralizan docs de módulos:

- `daemon/docs/*` → `docs/daemon/` (11 archivos)
- `mcps/memory/docs/postmortems/*` → `docs/mcp-memory/postmortems/`
- `agents/*` → `docs/agents/`
- `docs/README.md` (índice + convención extendida del SOT)

Convención v1.3 extendida:

| Tipo de doc | Ubicación |
|---|---|
| Governance | `.sdd/` (UPPERCASE) |
| Operation | `.sdd/methodologies/` (lowercase) |
| Module docs | `docs/<module>/` (lowercase) |

Los sub-repos NO deben tener su propia `docs/` (linkean al meta-repo).

## Issues pendientes del lint (T-12)

`bash bin/lint.sh` actualmente reporta 7 errores. Todos son **legacy
refs** en docs históricos (INDEXING.md, GOV-STRUCTURE.md, method-07,
method-05, EP-0007 docs) que documentan la convención o el cambio.
Soluciones:

1. **Check 2** (refs a directorios viejos): el grep es muy agresivo.
   Mejor: ignorar refs en líneas que contengan `(legacy)`,
   `(borrado en v1`, `(no usado`, `Removido del plan`, o `migraci`.
   La mayoría son refs DOCUMENTANDO el cambio histórico, no refs activas.

2. **Check 3** (refs a `constitution.md` en EP-0007 docs): las refs en
   `EP-0007/tasks.md` y `EP-0007/specs/.../requirements.md` son refs al
   ID histórico. Cambiar a `CONSTITUTION.md`.

3. **Lint se hace más inteligente** con esos filtros, debe pasar.

## Convenciones v1.3 (resumen para la sesión que sigue)

- **Governance docs** en `.sdd/` root → UPPERCASE
- **EP templates** (`proposal.md`, `design.md`, `tasks.md`,
  `requirements.md`) → lowercase
- **EP IDs** → sequential numeric (EP-0001, ..., EP-0007, próxima = EP-0008)
- **ADRs** → UPPERCASE (`ADR-NNNN-...md`)
- **Methodologies** → lowercase (`method-NN-...md`)
- **Module docs** → `docs/<module>/` en meta-repo, lowercase
- **Toda la doc vive en `.sdd/`** (governance) o **`docs/<module>/`** (module) — NUNCA en sub-repos

## Bugs encontrados y arreglados durante la sesión

1. `bin/start-epic.sh` y `bin/close-epic.sh`: bash arithmetic falla con
   ceros a la izquierda (`0008`). Fix: `10#$LAST` antes de la suma.
2. `bin/close-epic.sh` STATE.md update: regex python con `\( `
   (paréntesis escapado en raw string) no funciona. Fix: callback sin grupos.
3. `bin/close-epic.sh` STATE.md: `NEXT_ID="EP-0009"` (con prefix "EP-")
   + python agrega otro "EP-" → "EP-EP-0009". Fix: `NEXT_ID` es solo el número.
4. `bin/lint.sh` (este commit `f1922a3`): wildcards `**` se expanden en
   bash. Fix: usar `grep -qF` en lugar de `-qE`.

## Archivos clave para la sesión que sigue

| Path | Para qué |
|---|---|
| `.sdd/changes/EP-0007-sot-governance-hardening/proposal.md` | R-001..R-012 |
| `.sdd/changes/EP-0007-sot-governance-hardening/tasks.md` | T-1..T-12 (T-12 es T-12) |
| `.sdd/changes/EP-0007-sot-governance-hardening/specs/EP-0007-01-sot-root/requirements.md` | criterios de aceptación verificables |
| `.sdd/INDEXING.md` | convenciones v1.3 |
| `.sdd/GOVERNANCE.md` | handbook único del workspace |
| `.sdd/AGENTS.md` | entry point para agentes IA |
| `.sdd/GOV-STRUCTURE.md` | mapa de jerarquías del SOT |
| `.sdd/STATE.md` | estado actual (EP-0007 activa, EP-0008 próxima) |
| `.sdd/bin/lint.sh` | validador del SOT (9 checks) |
| `.sdd/bin/close-epic.sh` | script de cierre atómico (corregido) |
| `.sdd/bin/start-epic.sh` | script de creación de EP (corregido) |
| `docs/README.md` (meta-repo) | índice de module docs |
| `docs/daemon/`, `docs/mcp-memory/`, `docs/agents/` | module docs migrados |

## Cómo retomar la sesión

```bash
cd /home/andres_fernandez/projects/neurox/.sdd

# 1. Ver el estado
git status
git log --oneline -5
cat STATE.md

# 2. Ver el estado de la EP-0007
ls changes/EP-0007-sot-governance-hardening/

# 3. Correr el lint
bash bin/lint.sh
# (esperar 7 errores de legacy refs, arreglarlos)

# 4. Cuando el lint pase, cerrar la EP
bash bin/close-epic.sh EP-0007

# 5. Commit de cierre
git add changes/ archived/ STATE.md REGISTRO.md 2>/dev/null
git commit -m "docs(ssd): archive EP-0007-sot-governance-hardening"
```

## Version

- **1.0** (2026-08-29) — inicial, creado durante la sesión de la EP-0007
  - Resume el estado de la sesión cerrada
  - 4 convenciones v1.3 documentadas
  - 4 bugs encontrados y arreglados
  - 12 commits de la sesión listados en orden
  - Lista de archivos clave para la próxima sesión