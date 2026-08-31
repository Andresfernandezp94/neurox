# Method 08 — Doc-Update Mandatory (jerarquía SOT > Código > Doc)

> **Workflow obligatorio**: cualquier cambio (mayor o menor) en código debe
> propagar a la doc en el mismo PR. Un PR sin doc-update está incompleto.
>
> Versión: 1.0 (2026-08-29) | Status: active | Owner: todos
>
> Principio que enforce: **G12 en CONSTITUTION.md**.

## Propósito

Garantizar que el workspace `neurox` no acumule drift entre código y
documentación. Hoy (pre-EP-0009) hay cientos de referencias a
`adan`/`eva`/`default_agent`/`neurox_default` que ya no existen en
runtime, y nadie mantiene la doc en sync.

Este method es **vinculante**: cualquier PR que toque código de
producción sin propagar a la doc correspondiente NO se mergea.

## La jerarquía

```
┌─────────────────────────────────────────┐
│ SOT  (.sdd/)                            │  define CÓMO se trabaja
│   • CONSTITUTION, GOVERNANCE, GLOSSARY  │
│   • methodologies/                      │
│   • decisions/ (ADRs cross-repo)        │
└────────────────┬────────────────────────┘
                 │ guía
                 ▼
┌─────────────────────────────────────────┐
│ Código (sub-repos + meta-repo)          │  implementa lo que el SOT dice
│   • daemon/, client/*, mcps/*           │
│   • .sdd/bin/, scripts/                 │
└────────────────┬────────────────────────┘
                 │ debe documentarse
                 ▼
┌─────────────────────────────────────────┐
│ Doc  (docs/<module>/)                   │  describe lo que el código hace
│   • architecture.md                     │
│   • api.md                              │
│   • integrations/                       │
│   • decisions/ (module-scoped)          │
└─────────────────────────────────────────┘
```

## Workflow por nivel de cambio

### Cambio en código (cualquier magnitud)

Antes de abrir el PR:

1. **Identificar el módulo afectado**:
   - ¿Es daemon? → actualizar `docs/daemon/`
   - ¿Es client/web? → actualizar `docs/client-web/` (si existe) o `docs/daemon/architecture.md#client`
   - ¿Es mcps/X? → actualizar `docs/<module>-<mcp>/` (si existe) o `docs/daemon/integrations/<mcp>.md`
   - ¿Es meta-repo (bin/, scripts/)? → actualizar `.sdd/methodologies/` o `.sdd/bin/README.md`

2. **Identificar el tipo de cambio**:
   - **API pública** (endpoint, schema, response shape) → `docs/<module>/api.md`
   - **Arquitectura** (nuevo crate, refactor mayor, nuevo flujo) → `docs/<module>/architecture.md`
   - **Integración** (cómo se conecta con otro módulo) → `docs/<module>/integrations/<other>.md`
   - **Decisión local** (no cross-repo) → `docs/<module>/decisions/NNNN-<slug>.md`
   - **Bugfix** (comportamiento documentado que cambia) → `docs/<module>/<file>.md` + postmortem si es relevante

3. **Actualizar la doc en el mismo commit** (o commits separados en el mismo PR, todos antes del merge)

4. **Si la doc nueva no encaja en existente**: crear el archivo nuevo + agregar al `.sdd/DOCS-INDEX.md`

### Cambio en SOT (principios, convenciones, ADRs)

Más raro, pero si pasa:

1. Cambiar el archivo en `.sdd/` (CONSTITUTION, GOVERNANCE, GLOSSARY, etc.)
2. Si afecta naming/prefix: bumpear `INDEXING.md` con changelog
3. Si agrega/borra archivos del SOT: actualizar `DOCS-INDEX.md`
4. Si cambia workflow: actualizar `methodologies/` correspondiente

### Cambio solo en doc (sin tocar código)

Raro pero válido (e.g., fix typo, clarificar ejemplo). No requiere
propagación. Pero si el cambio revela que el código NO hace lo que la
doc dice → abrir ticket de bug.

## Checklist pre-PR

Antes de pedir review, confirmar:

- [ ] Si cambié código, ¿actualicé la doc del módulo?
- [ ] Si cambié API, ¿está en `api.md`?
- [ ] Si cambié convención (naming, prefix, status), ¿bumpeé `INDEXING.md`?
- [ ] Si agregué archivo nuevo en `docs/<module>/`, ¿lo indexé en `DOCS-INDEX.md`?
- [ ] Si cambié un ADR cross-repo, ¿se reflejó en `decisions/README.md`?
- [ ] Si toqué el SOT, ¿`bash .sdd/bin/lint.sh` exit 0?

## Enforcement

**Lint automático** (en CI):

- `bash .sdd/bin/lint.sh` debe pasar exit 0 antes de merge
- Check 1: frontmatter de EPs
- Check 2: refs rotas a directorios viejos
- Check 3: refs rotas a archivos viejos
- Checks 4-8: numeración, sync STATE/REGISTRO, naming UPPERCASE, DOCS-INDEX existe
- Check 10 (EP-0009): agent names no canónicos (`adan`/`eva`)
- Check 11 (EP-0009): `default_agent` o `neurox_default` como agent id literal
- Check 12 (EP-0009): `MiAgente` placeholder

**Code review**:

- Reviewer pregunta: "¿dónde está el doc-update?"
- Si la respuesta es "no necesita", el reviewer decide si es verdad
- Default: rechazar el PR hasta que incluya el doc-update

## Anti-patterns

❌ **NO** commitear código sin actualizar la doc correspondiente
❌ **NO** crear un PR grande con código + un PR separado con doc-update
❌ **NO** dejar refs a nombres viejos (`adan`/`eva`) "porque el archivo es histórico"
  → si es histórico, agregar banner pre-rename (ver EP-0009 A2)
❌ **NO** borrar docs sin verificar que la info está en otro lado (single source of truth)

## Ejemplos

### Ejemplo 1 — Cambio de binario

```
Cambio: rename binario `adan` → `agent` en daemon/

Doc-update requerido:
- docs/daemon/README.md (binarios listados)
- docs/daemon/production.md (deploy instructions)
- docs/daemon/api.md (si menciona el binario)
- .sdd/DOCS-INDEX.md (si hay entry sobre arquitectura de agents)
```

### Ejemplo 2 — Nuevo endpoint

```
Cambio: agregar POST /v1/agents/:id/start en daemon/

Doc-update requerido:
- docs/daemon/api.md (nueva fila en tabla de endpoints)
- CHANGELOG.md (entry con el nuevo endpoint)
```

### Ejemplo 3 — ADR cross-repo

```
Cambio: aceptar ADR-NNNN sobre streaming endpoint routing

Doc-update requerido:
- .sdd/decisions/ADR-NNNN-*.md (el ADR en sí)
- .sdd/decisions/README.md (tabla índice)
- .sdd/DOCS-INDEX.md (sección §2 ADRs)
```

## Relación con otras reglas

- **G12** (CONSTITUTION): principio duro de jerarquía
- **INDEXING.md** v1.5: convención de naming y prefijos
- **DOCS-INDEX.md**: catálogo canónico de toda la doc
- **method-04-workflow-prs.md**: workflow de PRs (este method se ejecuta dentro de ese workflow)

## Version

- **1.0** (2026-08-29) — inicial (creado en EP-0009 T-18)
  - Workflow obligatorio de propagación código → doc
  - Checklist pre-PR
  - Enforcement vía lint + code review
  - Anti-patterns explícitos
