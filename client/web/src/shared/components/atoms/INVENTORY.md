# Atoms Inventory — EP-0016

Snapshot captured 2026-08-08 from `src/shared/styles/common.css`
(2614 LoC) and all 23 non-test TSX files.

## Átomos identificados (7 + 1 derivado)

### A1 — Button

**CSS class actual**: `.btn`, `.btn-primary`, `.btn-secondary`,
`.btn-danger`, `.btn-sm`

**Uso**: 17 (`.btn`), 3 (`btn-primary`), 4 (`btn-secondary`),
1 (`btn-danger`), 7 (`btn-sm`), 2 (`btn--small` legacy)

**Inconsistencias detectadas**:
- `.btn-sm` y `.btn--small` conviven (BEM vs no-BEM)
- `.btn-primary` y `.btn--primary` conviven
- **Decisión**: unificar como BEM (`btn--primary`, `btn--secondary`,
  `btn--danger`, `btn--sm`, `btn--ghost`)

**Wrapper React**: SÍ
- Props: `variant` (primary | secondary | danger | ghost),
  `size` (sm | md), `disabled`, `loading`, `onClick`, `children`
- Justificación: onClick, disabled, loading son lógica

**Inline styles que migra**: 7 (`fontSize: "0.75rem"` aplicados al btn)

---

### A2 — Input

**CSS class actual**: `.input`

**Uso**: 4

**Wrapper React**: SÍ
- Props: `value`, `onChange`, `placeholder`, `disabled`, `ref`,
  `aria-label`, `size` (sm | md)
- Justificación: necesita ref para focus management

**Inline styles que migra**: 0 (no se usa con style inline)

---

### A3 — Label

**CSS class actual**: `.label-text`

**Uso**: 4 (solo en ProvidersPanel)

**Wrapper React**: SÍ
- Props: `htmlFor`, `children`, `required` (boolean)
- Justificación: label semántico con `htmlFor` necesita ser componente
  para a11y correcto

---

### A4 — Badge

**CSS class actual**: `.status-badge`, `.status-badge--active`,
`.status-badge--warn`, `.status-badge--danger`, `.status-badge--tools`,
`.status-badge__dot`, `.status-active`, `.status-warn`, `.status-danger`,
`.status-tools`

**Uso**: 3 (`status-badge`), 1 cada variant

**Inconsistencias detectadas**:
- Nombres con prefijo `status-` que NO son utility globales, son
  variants de un solo átomo
- Mezclan `.status-badge--active` (BEM) con `.status-active` (no-BEM)
- **Decisión**: renombrar a `.badge`, `.badge--success`, `.badge--warn`,
  `.badge--danger`, `.badge--info`, `.badge--neutral`, `.badge__dot`

**Wrapper React**: SÍ
- Props: `variant`, `dot` (boolean), `children`

---

### A5 — Icon

**CSS class actual**: `.icon-btn`, `.icon-btn--danger`

**Uso**: 2 (`icon-btn`)

**Wrapper React**: SÍ (sobre `Icons.tsx` existente)
- Props: `name`, `size` (sm | md | lg), `color`, `onClick` (cuando es
  button), `aria-label`

**Decisión**: NO crear un nuevo componente `<Icon>` — usar el wrapper
existente en `shared/components/Icons.tsx`. Solo crear `<IconButton>`
para botones que son solo iconos.

---

### A6 — IconButton

**CSS class actual**: `.icon-btn`, `.icon-btn--danger`

**Wrapper React**: SÍ (compone con A5 Icon)
- Props: `icon` (name), `aria-label`, `variant` (default | danger),
  `size`, `onClick`, `disabled`

---

### A7 — Spinner

**CSS class actual**: `.spinner`

**Uso**: 1

**Wrapper React**: SÍ (animación CSS keyframe)
- Props: `size` (sm | md | lg), `label` (a11y)

---

### A8 — Code (texto mono, inline)

**CSS class actual**: `.code`

**Uso**: 15

**Decisión**: NO wrapper React — utility class pura `text-mono`.
- Props: ninguna, es solo CSS
- Justificación: 15 usos sin lógica asociada

---

## Átomos derivados (text helpers, sin wrapper React)

Estos NO son átomos wrapper React, son utility classes que se usan
junto con otros átomos/moléculas.

| Utility | Uso | Descripción |
|---|---|---|
| `.muted` | 36 | Texto dim/secondary |
| `.dim` | 1 | Igual a muted pero más sutil |
| `.strong` | 7 | Texto primary bold |
| `.loading` | 2 | Texto italic "loading…" |
| `.text-right` | (nueva, 8 inline styles la necesitan) | text-align: right |
| `.text-sm` | (nueva, 7 inline styles) | font-size: 0.75rem |
| `.text-md` | (nueva, 3 inline styles) | font-size: 0.875rem |
| `.text-xs` | (nueva, 3 inline styles) | font-size: 0.6875rem |
| `.text-lg` | (nueva, 1 inline style) | font-size: 1rem |

**Total utility text helpers**: 9 nuevas classes para cubrir los 22
inline styles de text-align/font-size detectados.

---

## Átomos NO creados (decisión explícita)

- ❌ **Heading** (h1/h2/h3) — no se usa como clase, los paneles
  usan `<SectionHeader>` (molécula) o tipografía heredada
- ❌ **Avatar** — no se usa en el admin
- ❌ **Tooltip** — no implementado, fuera de scope
- ❌ **Divider** (hr) — no se usa
- ❌ **Switch / Toggle** — no se usa (theme switcher es custom)

---

## Inline styles TOP a eliminar (resumen)

| Inline style | Cuenta | Átomo que lo absorbe |
|---|---|---|
| `textAlign: "right"` | 8 | `.text-right` (text helper) |
| `fontSize: "0.75rem"` | 7 | `.text-sm` (text helper) |
| `marginBottom: "0.5rem"` | 4 | spacing scale (consumir via Stack gap) |
| `fontSize: "0.875rem"` | 3 | `.text-md` (text helper) |
| `display: flex, flexDirection: column, gap: 0.75rem` | 3 | `<Stack gap="md">` (molécula) |
| `marginBottom: "1rem"` | 2 | `<Stack gap="lg">` o spacing utility |
| `display: flex, flexDirection: column, gap: 0.5rem` | 2 | `<Stack gap="sm">` (molécula) |
| `flexShrink: 0` | 2 | `.flex-shrink-0` utility |
| `fontFamily: "var(--font-mono)"` | 2 | `.text-mono` utility |
| otros | ~80 | caso por caso |

**Cobertura estimada con átomos propuestos**: ~40 de 107 inline styles
pueden absorberse con utility classes + Stack. El resto requiere
wrapper React (Button, Card, Row, EmptyState, Badge) o son específicos
del organismo.

---

## Resumen ejecutivo

| Átomo | Wrapper React | CSS class | Migración estimada |
|---|---|---|---|
| A1 Button | ✅ | `.btn--*` | 8 inline styles |
| A2 Input | ✅ | `.input` | 1 |
| A3 Label | ✅ | `.label-text` | 0 |
| A4 Badge | ✅ | `.badge--*` | 2 |
| A5 Icon | ✅ | (wrapper) | (sin cambios) |
| A6 IconButton | ✅ | `.icon-btn--*` | 1 |
| A7 Spinner | ✅ | `.spinner` | 0 |
| A8 Code | ❌ (CSS only) | `.text-mono` | 5 |
| Text helpers | ❌ (CSS only) | `.muted`, `.dim`, `.strong`, `.text-sm`, `.text-md`, `.text-lg`, `.text-xs`, `.text-right`, `.loading` | ~30 |
| **TOTAL** | 7 wrappers | ~20 utility classes | ~47 inline styles absorbidos |

Quedan ~60 inline styles que se migran vía moléculas (Card, Row,
EmptyState, ErrorBanner) o son organismos-específicos.
