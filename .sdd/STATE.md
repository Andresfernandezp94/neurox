# STATE — SOT neurox

> **Machine-readable state** del SOT. Editar a mano solo si no hay
> alternativa — los scripts `bin/*` deberían ser la fuente primaria de
> updates.

## Épicas activas

<!-- Auto-updated by bin/start-epic.sh / bin/close-epic.sh -->

| # | Épica | Status | Owner | Próximo step |
|---|-------|--------|-------|--------------|

## Última épica numerada

_Ninguna._ El historial de épicas fue purgado para comenzar un nuevo
ciclo. La próxima épica libre es `EP-0001` — lo calcula
`bin/start-epic.sh` buscando el máximo en `changes/` y `archived/`
(ambos vacíos).

## Épicas cerradas

Ver [`archived/REGISTRO.md`](./archived/REGISTRO.md) — sin entradas por ahora.

## ADRs vigentes

Ver [`decisions/`](./decisions/) — 2 ADRs aceptados (ADR-0001, ADR-0002).

## Procesos de seguridad activos

Ver [`security/MANIFEST.md`](./security/MANIFEST.md) — auto-actualizado por `security/bin/start.sh`.

## Convenciones del SOT

- Prefijos de archivo y estados: ver [`INDEXING.md`](./INDEXING.md)
- Path convention (`@/` → workspace root): ver [`CONVENTION.md`](./CONVENTION.md)
- ADRs cross-repo: [`decisions/`](./decisions/)
- Cambio vs archivo: `changes/` (abiertas) vs `archived/` (cerradas)
