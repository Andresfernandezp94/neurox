/**
 * Lector de breakpoints para guards de CSS.
 *
 * `vitest.config.ts` tiene `css: false`: las hojas de estilo ni se procesan,
 * asi que ningun test de componente puede observar que una regla de media
 * query escondio (o mostro) algo. Cuando el bug es de CSS, el guard tiene
 * que leer el CSS.
 *
 * Este modulo hace eso sin depender de un parser de CSS completo: con el
 * indice de llaves alcanza, porque lo que interesa son los bloques de
 * nivel superior de cada `@media`, no la especificidad ni los valores.
 */

export type Reglas = Map<string, string>;

/** Reglas declaradas dentro de un breakpoint, por selector. */
export function reglasEnMediaQuery(css: string, maxWidth: number): Reglas {
  const out: Reglas = new Map();
  const ini = css.indexOf(`@media (max-width: ${maxWidth}px) {`);
  if (ini === -1) return out;

  // El bloque del media query: se avanza por profundidad de llaves para no
  // comerse los `@media` anidados.
  let i = css.indexOf("{", ini);
  let depth = 0;
  for (; i < css.length; i++) {
    const c = css[i];
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) break;
    }
  }

  for (const m of css
    .slice(ini, i)
    .matchAll(/(^|\n)\s*(\.[a-zA-Z0-9_-]+)\s*\{([^}]*)\}/g)) {
    const selector = m[2];
    const body = m[3];
    if (selector !== undefined && body !== undefined) out.set(selector, body);
  }
  return out;
}

/** `display: none` declarado en el cuerpo de una regla. */
export function esDisplayNone(cuerpo: string | undefined): boolean {
  return cuerpo !== undefined && /display:\s*none/.test(cuerpo);
}
