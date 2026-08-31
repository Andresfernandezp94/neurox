# EP-{{NNNN}} — Tasks

> **Status**: draft:tasks
> **Created**: {{YYYY-MM-DD}}

## Plan por fases

[Recomendado: dividir en 4 fases que correspondan a milestones verificables.
Si la EP tiene un solo componente, podés tener solo 2-3 fases.]

### Fase 1 — Setup

- [ ] Crear branch / preparar directorios
- [ ] Definir el schema del nuevo tipo (Rust + TS)
- [ ] Stub del endpoint con `return 200` + test mínimo

### Fase 2 — Implementación

- [ ] Handler real en el daemon
- [ ] Cliente TS (api/…)
- [ ] Componente React que consume el endpoint
- [ ] Integración con `StoreProvider` para reactivo

### Fase 3 — Tests

- [ ] Unit tests del handler (status, payload, error paths)
- [ ] Test de integración E2E con daemon + SPA
- [ ] Test del contrato (qué pasa si el schema cambia)
- [ ] Coverage >80% del módulo nuevo

### Fase 4 — Docs

- [ ] README.md del repo afectado (si aplica)
- [ ] CHANGELOG.md entry
- [ ] ADR si la decisión arquitectural lo amerita
- [ ] Anunciar en el canal correspondiente (si hay)

## Dependencias cruzadas

[OPTIONAL: Otras épicas o sistemas externos.]
