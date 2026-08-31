// useVisualViewportHeight — robust mobile keyboard handling
//
// En Android Chrome (sobre todo <108) y otros browsers móviles,
// `100dvh` devuelve la altura del LAYOUT viewport (sin descontar
// el teclado virtual). El resultado: al abrir el teclado, queda
// una banda visible entre el bottom del shell y el top del teclado,
// o el contenido se queda tapado por el teclado y no "sube".
//
// `window.visualViewport` (CSSOM View Module) SÍ reporta el tamaño
// del área realmente visible, descontando teclado, bars de UI del
// navegador que aparecen/desaparecen, etc.
//
// Este hook:
//   1. Si window.visualViewport existe: escucha resize/scroll y
//      actualiza la CSS var --app-h con la altura reportada.
//      Cualquier elemento que use var(--app-h, 100dvh) se ajustará
//      en vivo.
//   2. Si no existe (SSR, desktop sin support): no hace nada → el
//      CSS usa el fallback 100dvh.
//
// SSR-safe: en el primer render no toca el DOM. Listener y cleanup
// en useEffect.

import { useEffect } from "react";

const VAR_NAME = "--app-h";

function readHeight(): number | null {
  if (typeof window === "undefined") return null;
  if (typeof window.visualViewport === "object" && window.visualViewport !== null) {
    return window.visualViewport.height;
  }
  return null;
}

export function useVisualViewportHeight(): void {
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (typeof window.visualViewport !== "object" || window.visualViewport === null) {
      // Sin soporte: dejamos --app-h sin definir → el CSS usa 100dvh como fallback.
      return;
    }

    const apply = () => {
      const h = readHeight();
      if (h === null || !Number.isFinite(h) || h <= 0) return;
      document.documentElement.style.setProperty(VAR_NAME, `${Math.round(h)}px`);
    };

    apply();
    const vv = window.visualViewport;
    vv.addEventListener("resize", apply);
    vv.addEventListener("scroll", apply);
    window.addEventListener("orientationchange", apply);
    window.addEventListener("resize", apply);

    return () => {
      vv.removeEventListener("resize", apply);
      vv.removeEventListener("scroll", apply);
      window.removeEventListener("orientationchange", apply);
      window.removeEventListener("resize", apply);
    };
  }, []);
}
