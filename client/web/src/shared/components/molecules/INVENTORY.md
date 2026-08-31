# Molecules Inventory — EP-0016

Snapshot captured 2026-08-08 from `src/shared/styles/common.css`
(2614 LoC) and all 23 non-test TSX files.

## Moléculas identificadas (6)

### M1 — Card

**CSS class actual**: `.card`, `.card-body`, `.card-title`, `.card__body`,
`.card__actions`

**Uso**: 11 (`.card`), 1 (`card-title`), 1 (`card-body`)

**Inconsistencias detectadas**:
- `.card-body` y `.card__body` conviven (BEM vs no-BEM)
- **Decisión**: unificar como BEM → `.card`, `.card__body`,
  `.card__title`, `.card__actions`

**Wrapper React**: SÍ (compound)
- API:
  ```tsx
  <Card>
    <Card.Title>Service Health</Card.Title>
    <Card.Body>...</Card.Body>
    <Card.Actions>...</Card.Actions>
  </Card>
  ```
- Props del Card: `children`, `className`, `as` (polymorphic)
- Justificación: composición con sub-elementos reutilizables

**Inline styles que migra**: varios `padding`, `background` aplicados a
contenedores que terminan siendo Cards.

---

### M2 — Panel

**CSS class actual**: `.panel`, `.panel-header`

**Uso**: 7 (`.panel`)

**Decisión**: `Panel` es una variante de `Card` con padding distinto y
header obligatorio. **Reusar Card con variant="panel"**.

- API: `<Panel><Panel.Header title="..." />...</Panel>`
- Props: `title`, `actions`, `children`

**Inline styles que migra**: ninguno directo, pero reduce duplicación
de headers custom en cada panel.

---

### M3 — Row

**CSS class actual**: `.row`, `.row-between`

**Uso**: 8 (`.row`), 7 (`.row-between`)

**Wrapper React**: SÍ
- Props: `justify` (start | between | end | around), `align` (start |
  center | end), `gap` (sm | md | lg), `wrap` (boolean)
- Justificación: props simples, consume utility classes existentes

---

### M4 — Stack

**CSS class actual**: `.stack`, `.stack-lg`

**Uso**: 1 cada uno

**Wrapper React**: SÍ
- Props: `gap` (sm | md | lg), `direction` (column | row),
  `align`, `justify`
- Justificación: este es el atajo que más inline styles absorbe
  (`display: flex; flexDirection: column; gap: ...` aparece 5+ veces)

**Inline styles que migra**: 5+ (todos los `display: flex;
flexDirection: column; gap: ...`)

---

### M5 — EmptyState

**CSS class actual**: `.empty`, `.empty-title`, `.empty-hint`

**Uso**: 4 (`.empty`), 4 (`.empty-title`), 1 (`.empty-hint`)

**Wrapper React**: SÍ (compound)
- API:
  ```tsx
  <EmptyState>
    <EmptyState.Icon name="inbox" />
    <EmptyState.Title>No items</EmptyState.Title>
    <EmptyState.Hint>Try adjusting your filters</EmptyState.Hint>
  </EmptyState>
  ```

**Inline styles que migra**: el componente `.empty` ya está usado en
StatusPanel, AgentsPanel, etc.

---

### M6 — ErrorBanner

**CSS class actual**: `.error-banner`

**Uso**: 5

**Wrapper React**: SÍ
- Props: `children`, `variant` (error | warn | info), `onDismiss`
  (opcional)

---

### M7 — SearchBar (derivada)

**CSS class actual**: `.session-list__search`, `.chat-history__search`
(BEM scoped, no utility)

**Uso**: varios en SessionList y ChatHistory

**Wrapper React**: SÍ
- Props: `value`, `onChange`, `placeholder`, `onClear`, `icon`
- Justificación: 2 organismos reimplementan search UI casi idéntica

---

### M8 — StatPair (label + value) (derivada)

**CSS class actual**: `.session-list__stat`, `.session-list__stat--total`,
`.session-list__stat--active`, `.session-list__stat--closed`

**Uso**: 3 (`.session-list__stat`)

**Wrapper React**: SÍ
- Props: `label`, `value`, `variant` (default | active | closed | total)
- Justificación: 3 variantes de un mismo componente "stat"
- Reemplaza las 3 instancias en SessionList header

---

## Inline styles adicionales absorbidos por moléculas

| Inline style | Cuenta | Molécula destino |
|---|---|---|
| `margin: "2rem auto", maxWidth: "32rem"` | 1 | `<EmptyState>` variant="centered" |
| `margin: "0 0 1rem"` | 1 | `<Stack gap="lg">` o spacing utility |
| `listStyle: "none", padding: 0, margin: 0` | 1 | `<Stack as="ul">` con reset |
| `overflow: hidden, textOverflow: ellipsis, whiteSpace: nowrap` | 1 | `<Text truncate>` utility |

---

## Moléculas NO creadas (decisión explícita)

- ❌ **Toolbar** (fila con título + acciones a la derecha) — los paneles
  usan SectionHeader existente, suficiente
- ❌ **Modal/Dialog** — ya existen `AlertModal`, `ConfirmDialog`,
  `SessionDetailModal` (organismos)
- ❌ **Tabs** — el admin no usa tabs (solo el sidebar principal)
- ❌ **Tooltip** — no implementado
- ❌ **Accordion** — usado internamente en ToolsExplorer, no es reusable
- ❌ **Toast/Notification** — no implementado (ErrorBanner cumple)

---

## Resumen ejecutivo

| Molécula | Wrapper React | Inline styles absorbidos | Componentes reemplazados |
|---|---|---|---|
| M1 Card | ✅ compound | varios | paneles con `.card` |
| M2 Panel | ✅ (variant de Card) | 0 directo | 7 paneles |
| M3 Row | ✅ | 2 (`flexShrink`, etc) | StatusPanel, ConfigViewer |
| M4 Stack | ✅ | 5+ | StatusPanel, LogsPanel, etc |
| M5 EmptyState | ✅ compound | 1 | StatusPanel, AgentsPanel, LogsPanel, etc |
| M6 ErrorBanner | ✅ | 0 directo | 5 paneles |
| M7 SearchBar | ✅ | 2 (`.session-list__search`, `.chat-history__search`) | SessionList, ChatHistory |
| M8 StatPair | ✅ | 0 directo | SessionList |
| **TOTAL** | 8 wrappers | ~10 | 8+ organismos |

Cobertura estimada total F2+F3: **~57 inline styles absorbidos** de los
107 detectados (53%). Los 50 restantes son organismos-específicos o
spacing puntual que se resuelve en F4 panel por panel.
