# Metodologías del Workspace

> **Índice de metodologías operativas** del workspace `neurox`. Convenciones y reglas para trabajo día a día.
>
> **Entry point**: [../GOVERNANCE.md](../GOVERNANCE.md) es el handbook único que explica **cómo se trabaja** en este repo. Leelo primero.
>
> Ver también: [INDEXING.md](../INDEXING.md) para las convenciones de prefijos y estados del SOT.

## Convenciones aplicadas

Cada archivo en este directorio sigue el prefijo `method-NN-` donde `NN` es el número de orden de lectura (2 dígitos zero-padded).

## Índice de metodologías

| # | Doc | Propósito | Status |
|---|-----|-----------|--------|
| 01 | [method-01-flujo-de-trabajo.md](./method-01-flujo-de-trabajo.md) | Cómo trabajar día a día en cualquier repo del workspace | draft |
| 02 | [method-02-buenas-practicas-documentacion.md](./method-02-buenas-practicas-documentacion.md) | Cómo escribir docs que no se desactualicen | draft |
| 03 | [method-03-buenas-practicas-codigo.md](./method-03-buenas-practicas-codigo.md) | Reglas de estilo, naming, errores, comentarios | draft |
| 04 | [method-04-workflow-prs.md](./method-04-workflow-prs.md) | Cómo abrir, revisar y mergear PRs | draft |
| 05 | [method-05-checklist-release.md](./method-05-checklist-release.md) | Pasos para taggear y publicar una versión | draft |
| 06 | [method-06-convenciones-git.md](./method-06-convenciones-git.md) | Reglas de branches, commits, tags, submodules | draft |
| 07 | [method-07-estados-y-steps.md](./method-07-estados-y-steps.md) | Máquina de estados para tracking de features/épicas/fixes | draft |
| 08 | [method-08-doc-update-mandatory.md](./method-08-doc-update-mandatory.md) | Workflow de propagación código → doc (jerarquía G12) | active |

## Estado global

- **Status agregado**: 7 drafts, 0 en review, 0 active
- **Coverage**: workflow diario (sí), PRs (sí), releases (sí), git (sí), tracking de estados (sí)
- **Faltantes documentados** (en INDEXING.md):
  - Metodología de seguridad → vive en [../security/](../security/) (proceso agent-agnostic)
  - Metodología de desarrollo → bloqueada por feature activa en `neuro-pro/`
- **Cross-ref**: cada método-NN-* está linkeado desde [../GOVERNANCE.md §13](../GOVERNANCE.md#13-índice-de-metodologías-deep-dives) como deep dive.

## Cómo agregar una nueva metodología

1. Asignar siguiente número correlativo (ver último en la tabla arriba)
2. Crear `method-NN-<slug>.md` con el frontmatter estándar (ver INDEXING.md)
3. Agregar entrada a la tabla de este README
4. Commit con mensaje: `docs(ssd): add method-NN <título>`

## Cómo actualizar una metodología existente

- **Status `draft`**: libre edición
- **Status `review`**: requiere review de al menos 1 persona antes de promover a `active`
- **Status `active`**: ediciones requieren justificar en el frontmatter
- **Status `superseded`**: marcar en el frontmatter, linkear al sucesor

## Versionado

- **Major bump**: cambio de estructura de directorios o sistema de prefijos
- **Minor bump**: agregar o quitar metodologías completas
- **Patch bump**: correcciones o clarificaciones a metodologías existentes

Ver changelog al final de cada archivo individual para su historial.