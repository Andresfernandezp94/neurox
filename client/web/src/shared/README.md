# src/shared/ — Port one-way de agent-studio

Esta carpeta contiene un **port one-way** (copia estática) de los componentes visuales de agent-studio, adaptados al admin del daemon neurox.

## Source of port

- **Origen**: `~/Laboratory/Projects/repositories/agent-studio/src-ui/`
- **Snapshot del commit**: el port se hace desde un commit específico de agent-studio y NO se sincroniza automáticamente.
- **Fecha del port inicial**: 2026-08-04 (EP-0002)

## Mapeo de archivos portados

| Source (agent-studio) | Dest (admin) | Notas |
|---|---|---|
| `modules/shared/styles/tokens.css` | `styles/tokens.css` | CSS variables (dark + light) |
| `modules/shared/styles/global.css` | `styles/theme.css` | Reset + body base |
| `modules/shared/styles/sidebar.css` | `styles/sidebar.css` | Estilos del Sidebar |
| `modules/shared/styles/status-bar.css` | `styles/status-bar.css` | Estilos del StatusBar |
| `modules/shared/styles/error-boundary.css` | `styles/error-boundary.css` | Estilos del ErrorBoundary |
| `modules/shared/hooks/useTheme.ts` | `hooks/useTheme.ts` | (idéntico) |
| `modules/shared/hooks/useI18n.ts` | `hooks/useI18n.ts` | (idéntico) |
| `modules/shared/i18n/es.json` | `i18n/es.json` | Solo keys que usa el admin |
| `components/shared/Sidebar.tsx` | `components/Sidebar.tsx` | Adaptado a 7 items (no 4) |
| `components/shared/Icons.tsx` | `components/Icons.tsx` | Solo iconos necesarios |
| `components/shared/SectionHeader.tsx` | `components/SectionHeader.tsx` | (idéntico) |
| `components/shared/StatusBar.tsx` | `components/StatusBar.tsx` | (idéntico) |
| `components/shared/ErrorBoundary.tsx` | `components/ErrorBoundary.tsx` | (idéntico) |
| `components/shared/ConfirmDialog.tsx` | `components/ConfirmDialog.tsx` | (idéntico) |
| `components/shared/AlertModal.tsx` | `components/AlertModal.tsx` | (idéntico) |

## Decisión: port one-way (no workspace compartido)

Ver [ADR-0005](../../../decisions/0005-port-one-way-agent-studio-ui.md) para el rationale completo.

**Resumen**:
- agent-studio es app desktop (Tauri 2), neurox/admin es web pura.
- Compartir package agregaría complejidad de monorepo innecesaria.
- El port one-way es simple: los archivos viven standalone en el admin.

## Cómo re-portar (si agent-studio evoluciona)

Si agent-studio cambia componentes que están portados acá:

1. Identificar el diff en agent-studio.
2. Aplicar el mismo diff manualmente en los archivos correspondientes del admin.
3. Verificar que los tests del admin siguen pasando.
4. Commitear con mensaje `port(shared): re-port from agent-studio @ <commit-hash>`.

**NO** usar git merge desde agent-studio. **NO** intentar sincronización automática.

## Cómo extender

Si necesitás un componente de agent-studio que NO está portado:

1. Verificar que está en la lista de `RF-04` o `RF-05` del spec [EP-0002-01](../../changes/EP-0002-admin-look-and-feel-agent-studio/specs/EP-0002-01-admin-look-and-feel-port/requirements.md).
2. Si está, agregarlo al port actualizando el mapeo de arriba.
3. Si no está, abrir nueva RF en el spec o crear nueva spec.

## Tests

Cada componente portado tiene su test correspondiente. Los tests están junto al código (`ComponentName.test.tsx`).

## Contexto

- Épica: [EP-0002 — Admin Look & Feel agent-studio](../../changes/EP-0002-admin-look-and-feel-agent-studio/proposal.md)
- Spec: [EP-0002-01 — Admin Look & Feel port](../../changes/EP-0002-admin-look-and-feel-agent-studio/specs/EP-0002-01-admin-look-and-feel-port/requirements.md)
- ADRs relacionados:
  - [ADR-0005 — Port one-way](../../decisions/0005-port-one-way-agent-studio-ui.md)
  - [ADR-0006 — Tailwind → CSS tokens](../../decisions/0006-replace-tailwind-with-css-tokens.md)
  - [ADR-0007 — Sidebar 7 items](../../decisions/0007-sidebar-with-7-items.md)
