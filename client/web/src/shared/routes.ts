// routes.ts — mapa de rutas de neurox-web.
//
// Rutas separadas por URL real para que no se pisen y cada una sea
// enlazable / recargable por separado:
//
//   /       landing pública (presentación del producto)
//   /login  pantalla de login
//   /app    panel de administración (tras login)
//
// Antes el flujo landing→login→admin vivía en un `useState` booleano
// dentro de App.tsx: todo era la misma URL, así que no había forma de
// linkear al login directo ni de recargar en el login sin perder el
// contexto.

export const ROUTES = {
  /** Landing pública. */
  landing: "/",
  /** Formulario de login. */
  login: "/login",
  /** Panel interno. */
  app: "/app",
} as const;

export type Route = (typeof ROUTES)[keyof typeof ROUTES];

const ALL_ROUTES: string[] = Object.values(ROUTES);

/**
 * Normaliza un pathname a una ruta conocida.
 *
 * Tolera trailing slash (`/login/` → `/login`) y cualquier subruta, que
 * caen a la landing: un deep link roto no debería dejar la app en blanco.
 */
export function normalizePath(pathname: string): Route {
  // Base path de Vite en producción (el daemon sirve el bundle desde un
  // subdirectorio). En dev es "/" y no hace nada.
  const base = (import.meta.env.BASE_URL ?? "/").replace(/\/$/, "");
  let p = pathname;
  if (base && p !== base && p.startsWith(base)) p = p.slice(base.length) || "/";

  const withoutQuery = p.split("?")[0] ?? "/";
  const clean = withoutQuery.replace(/\/+$/, "") || "/";
  return (ALL_ROUTES.includes(clean) ? clean : ROUTES.landing) as Route;
}

/** Ruta actual del navegador, ya normalizada. */
export function currentRoute(): Route {
  return normalizePath(window.location.pathname);
}

/**
 * Navega a una ruta. `replace: true` evita apilar historial — se usa
 * para login→app y logout→login, que no debería dejar el usuario
 * volviendo atrás a la pantalla anterior con el botón del navegador.
 */
export function navigate(to: Route, options: { replace?: boolean } = {}) {
  const base = (import.meta.env.BASE_URL ?? "/").replace(/\/$/, "");
  const href = `${base}${to}` || "/";
  if (options.replace) {
    window.history.replaceState({}, "", href);
  } else {
    window.history.pushState({}, "", href);
  }
  // Notifica a los hooks suscritos (useRoute), que noVen los eventos.
  window.dispatchEvent(new PopStateEvent("popstate"));
}