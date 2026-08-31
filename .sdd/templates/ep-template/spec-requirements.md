# EP-{{NNNN}}-01 — Spec para {{repo}}

> **Status**: draft:spec
> **Created**: {{YYYY-MM-DD}}
> **Repo**: {{repo}}

[Las specs son el **nivel granular** del OpenSpec: una EP grande tiene varias
specs, cada una enfocada en un repo o subsistema. Al crear una EP con N
componentes, se abren N specs (EP-NNNN-01-<repoA>, EP-NNNN-02-<repoB>, …).
Si la EP toca un solo repo, una sola spec.]

## Contexto

[REQUIRED: Qué parte del codebase afecta esta spec. Pegá paths reales.]

Ejemplo:
> Esta spec afecta `daemon/core/src/router/http.rs` (handler nuevo +
> schema), `client/web/src/api/foo.ts` (cliente TS), y un nuevo componente
> en `client/web/src/components/FooPanel.tsx`.

## Requisitos

[REQUIRED: Lista de requisitos. Cada uno con criterios verificables.]

### R-001 — {{título}}

**Como** {{rol}}
**Quiero** {{acción}}
**Para** {{beneficio}}

**Criterios de aceptación**:
- [ ] {{criterio 1}}
- [ ] {{criterio 2}}
- [ ] {{criterio 3}}

### R-002 — {{título}}

[Si aplica. Repetir estructura.]

## Out of scope

[OPTIONAL: Qué NO entra en esta spec. Ej: "el rate limiting es OUT —
eso es EP-NNNN-XYZ".]

- Rate limiting → EP-NNNN-04
- UI styling del panel → dependemos de design system, no diseñamos acá
