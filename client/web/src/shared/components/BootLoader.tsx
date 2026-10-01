// BootLoader — pantalla de arranque de neurox.
//
// Primer componente de la página "desde cero": fondo #020202 y el SVG de
// neurox (hexágono + nodo central + 3 nodos periféricos) centrado.
//
// El logo ya trae un gradiente con brillo metalizado animado
// (AppLogo), así que acá solo se compone el escenario: halo que respira,
// wordmark y un status opcional.
//
// Accesibilidad: el loader es puramente decorativo, por eso va
// `aria-hidden` y el texto real va en un `role="status"` aparte. Si el
// usuario pide menos movimiento, las animaciones se desactivan.

import { AppLogo } from "./AppLogo";

export interface BootLoaderProps {
  /** Mensaje de estado bajo el logo. Si se omite, no se renderiza. */
  status?: string;
}

export function BootLoader({ status }: BootLoaderProps) {
  return (
    <div className="boot-loader" data-testid="boot-loader">
      <div className="boot-loader__stage">
        <div className="boot-loader__halo" aria-hidden="true" />

        <AppLogo className="boot-loader__logo" />

        <span className="boot-loader__wordmark" aria-hidden="true">
          neurox
        </span>

        {status && (
          <span className="boot-loader__status" role="status">
            {status}
          </span>
        )}
      </div>
    </div>
  );
}