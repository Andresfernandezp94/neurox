// Lector de breakpoints para guards de CSS.
//
// `vitest.config.ts` tiene `css: false`: las hojas de estilo ni se procesan,
// asi que ningun test de componente puede observar que una regla de media
// query escondio (o mostro) algo. Cuando el bug es de CSS, el guard tiene
// que leer el CSS.
//
// Este modulo hace eso sin depender de un parser de CSS completo: con el
// indice de llaves alcanza, porque lo que interesa son los bloques de
// nivel superior de cada `@media`, no la especificidad ni los valores.

export type Reglas = Map<string, string>;

/**
 * Quita los comentarios.
 *
 * Necesario antes de parsear: un selector se reconoce como "todo lo que hay
 * entre el `}` anterior y el `{` siguiente", y un comentario es texto que
 * vive exactamente ahi. Sin esta limpieza, un comentario seguido de
 * `.user-info {` se lee como un solo selector que empieza por el comentario,
 * y la regla real nunca aparece en el mapa —el guard pasaria sin estar
 * mirando nada.
 *
 * Es una limpieza ingenua y a proposito: las hojas de estilo del proyecto no
 * usan comentarios anidados, y respetarlos justificaria un parser de verdad.
 */
function sinComentarios(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Reglas declaradas dentro de un breakpoint, por selector. */
export function reglasEnMediaQuery(css: string, maxWidth: number): Reglas {
  const out: Reglas = new Map();
  const limpio = sinComentarios(css);
  const ini = limpio.indexOf(`@media (max-width: ${maxWidth}px) {`);
  if (ini === -1) return out;

  // El bloque del media query: se avanza por profundidad de llaves para no
  // comerse los `@media` anidados.
  let i = limpio.indexOf("{", ini);
  let depth = 0;
  for (; i < limpio.length; i++) {
    const c = limpio[i];
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) break;
    }
  }

  // Selectores compuestos y de lista: `a .b`, `a:hover`, `a, b`. Se toma todo
  // lo que hay antes de `{` y se parte por comas. Cada selector se normaliza
  // a un solo espacio porque en el archivo los selectores largos se parten
  // en varias lineas.
  const re = /([^{}@]+?)\s*\{([^}]*)\}/g;
  for (const m of limpio.slice(ini, i).matchAll(re)) {
    const lista = m[1];
    const body = m[2];
    if (lista === undefined || body === undefined) continue;
    for (const sel of lista.split(",")) {
      const norm = sel.trim().replace(/\s+/g, " ");
      if (norm.startsWith(".")) out.set(norm, body);
    }
  }
  return out;
}

/** `display: none` declarado en el cuerpo de una regla. */
export function esDisplayNone(cuerpo: string | undefined): boolean {
  return cuerpo !== undefined && /display:\s*none/.test(cuerpo);
}
