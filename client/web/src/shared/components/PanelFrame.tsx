// PanelFrame.tsx — wrapper estándar para todos los paneles del admin.
//
// Compensa el header fixed con marginTop: var(--header-h) y toma el resto
// del alto disponible dentro del .app-main (flex column padre) con
// flex: 1 + minHeight: 0. El border 2px dashed currentColor es solo para
// iterar (pasale showBorder=false para ocultarlo).
//
// EP-0026-UX: el panel NO tiene padding. Las tabs de cada vista quedan
// pegadas al borde. El padding se aplica solo al contenido via la
// utility class `.page-pad` (1rem 1.5rem) en el wrapper del contenido.
//
// Uso estándar de una vista normal:
//   <PanelFrame testId="status-panel">
//     <div className="status-panel__tabs">...</div>
//     <div className="page-pad">
//       <Stack gap="md">...</Stack>
//     </div>
//   </PanelFrame>
//
// Recibe children opcionales. Cualquier style/className del consumidor
// se mergea sobre los defaults (tienen prioridad).

import type { CSSProperties, ReactNode } from "react";

interface PanelFrameProps {
  children?: ReactNode;
  testId?: string;
  className?: string;
  showBorder?: boolean;
  style?: CSSProperties;
}

export function PanelFrame({
  children,
  testId,
  className,
  showBorder = false,
  style,
}: PanelFrameProps) {
  return (
    <div
      data-testid={testId}
      className={className}
      style={{
        border: showBorder ? "2px dashed currentColor" : undefined,
        width: "100%",
        flex: 1,
        minHeight: 0,
        marginTop: "var(--header-h)",
        ...style,
      }}
    >
      {children}
    </div>
  );
}
