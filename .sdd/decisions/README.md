# ADRs — Architecture Decision Records

Decisiones arquitecturales formales del workspace `neurox`. **Inmutables** una vez aceptadas. Se superseden con nuevos ADRs que las referencian explícitamente.

## Convención

- **Filename**: `NNNN-slug-kebab-case.md` (4 dígitos zero-padded)
- **Lifecycle**: `proposed → accepted → (superseded by NNNN)`

## Índice

| # | Título | Status | Fecha | Épica |
|---|--------|--------|-------|-------|
| [0001](ADR-0001-multi-agent-subprocess-architecture.md) | Multi-agent subprocess architecture (one subprocess per session per agent) | accepted | 2026-08-15 | [EP-0004](../archived/EP-0004-multi-agent-live-switch/) |
| [0002](ADR-0002-streaming-endpoint-routing.md) | Streaming endpoint routes through `dispatch_to_agent`, not `llmd` | accepted | 2026-08-15 | [EP-0004](../archived/EP-0004-multi-agent-live-switch/) |

## Cómo agregar un nuevo ADR

1. Asignar siguiente número correlativo (ver último en la tabla)
2. Crear `NNNN-<slug-kebab-case>.md` con el frontmatter estándar
3. Agregar entrada a la tabla de este README
4. Commit con mensaje: `docs(sdd): propose ADR-NNNN <título>`
5. Una vez mergeado, marcar como `accepted` en frontmatter

## Cross-references

- Ver [INDEXING.md](../INDEXING.md) para la convención completa de prefijos y estados
- Ver [README.md](../README.md) para el entry point del SOT
- Ver [CONSTITUTION.md](../CONSTITUTION.md) para los principios globales
- Ver [GLOSSARY.md](../GLOSSARY.md) para vocabulario canónico
