// GridBackdrop — grilla decorativa de fondo con desvanecido radial.
//
// Origen: venía inline en `LoginScreen` (`<div class="login-screen__bg">
// <div class="login-screen__grid">`). Se extrajo a un átomo para que el
// login y la landing compartan la misma definición en vez de duplicar el
// CSS de las gradientes y el mask.
//
// Es puramente decorativo: no captura clicks (`pointer-events: none`) y
// va `aria-hidden`, así que no interrumpe la navegación por teclado ni
// la lectura de pantalla.
//
// La intensidad se controla con `--grid-line-color`, que el consumidor
// define en su scope. El default usa `color-mix` sobre `--accent`, que
// ya es distinto por tema — por eso responde al toggle sin CSS extra.

import type { CSSProperties } from "react";

export interface GridBackdropProps {
  /**
   * Cubre el viewport completo en vez de su contenedor, ignorando el
   * scroll. Es lo que se usa para el fondo general de la app: una sola
   * instancia en la raíz en lugar de una por vista.
   */
  fixed?: boolean;
  /**
   * Color de las líneas. Por defecto `color-mix` del accent al 35%.
   * Para bajar la intensidad: `color-mix(in oklab, var(--accent) 20%, transparent)`.
   */
  lineColor?: string;
  /** Grosor de las líneas. Default 2px. */
  lineWidth?: string;
  /** Tamaño de la celda. Default 3rem. */
  cellSize?: string;
  /** Dónde está el foco del desvanecido radial. Default centro. */
  fadeOrigin?: string;
  /** Hasta dónde se ve la grilla (0-100%). Default 70%. */
  fadeEnd?: number;
  className?: string;
  style?: CSSProperties;
}

export function GridBackdrop({
  fixed = false,
  lineColor = "color-mix(in oklab, var(--accent) 35%, transparent)",
  lineWidth = "2px",
  cellSize = "3rem",
  fadeOrigin = "center",
  fadeEnd = 70,
  className = "",
  style,
}: GridBackdropProps) {
  const classes = ["grid-backdrop", fixed ? "grid-backdrop--fixed" : "", className]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes} style={style} aria-hidden="true">
      <div
        className="grid-backdrop__lines"
        style={
          {
            "--grid-line-color": lineColor,
            "--grid-line-width": lineWidth,
            "--grid-cell-size": cellSize,
            "--grid-fade-origin": fadeOrigin,
            "--grid-fade-end": `${fadeEnd}%`,
          } as CSSProperties
        }
      />
    </div>
  );
}