// useRoute.ts — router mínimo sobre la History API.
//
// Suficiente para las 3 rutas de neurox-web y sin dependencia extra. Se
// usa `popstate` para que el botón atrás/adelante del navegador funcione,
// más un evento sintético desde `navigate()` porque `pushState` no emite
// ninguno por sí solo.
//
// Cuando se rechaza una dependencia como react-router se acepta que esto NO
// cubre: rutas anidadas, params, layouts anidados, scroll restoration,
// carga de datos por ruta, transiciones. Si la app crece hacia eso, esto
// hay que reemplazarlo por un router real.

import { useCallback, useEffect, useState } from "react";
import { currentRoute, navigate, type Route } from "../routes";

export interface Router {
  route: Route;
  /** Navega y actualiza el estado local. */
  go: (to: Route, options?: { replace?: boolean }) => void;
}

export function useRoute(): Router {
  const [route, setRoute] = useState<Route>(() => currentRoute());

  useEffect(() => {
    const sync = () => setRoute(currentRoute());
    // `popstate` cubre el botón atrás/adelante. `navigate()` lo emite a
    // mano porque pushState no dispara ninguno.
    window.addEventListener("popstate", sync);
    return () => window.removeEventListener("popstate", sync);
  }, []);

  const go = useCallback((to: Route, options?: { replace?: boolean }) => {
    navigate(to, options);
  }, []);

  return { route, go };
}