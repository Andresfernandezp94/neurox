import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/**
 * Paridad de tokens entre los dos temas.
 *
 * El bug que cubre: `--surface-deep` estaba definido solo en el bloque del
 * tema oscuro. En `[data-theme="light"]` no habia override, asi que
 * `var(--surface-deep)` resolvia a transparente y el cuerpo de todas las
 * cards se quedaba sin fondo en light mode (`.card__body` en
 * `molecules.css`), igual que `.plugin-card__tools` en `atoms.css` y las
 * filas del panel de Sandbox.
 *
 * CSS no avisa cuando un `var()` no existe en el tema activo: la regla
 * compila, se aplica, y el fondo simplemente no esta. Por eso hace falta
 * un chequeo explicito de que todo token de color definido en el tema
 * base tenga su contraparte en claro.
 */

const CSS_PATH = join(dirname(fileURLToPath(import.meta.url)), "tokens.css");

type Block = { selector: string; body: string };

/** Parser de bloques de nivel superior. Soporta comentarios y anidamiento
 *  (los inner blocks de `@media` se ignoran: no redefinen tokens de tema,
 *  salvo que se pruebe lo contrario y se actualice esto). */
function topLevelBlocks(css: string): Block[] {
  const sinComentarios = css.replace(/\/\*[\s\S]*?\*\//g, " ");
  const out: Block[] = [];
  let depth = 0;
  let selStart = 0;
  let bodyStart = -1;
  for (let i = 0; i < sinComentarios.length; i++) {
    const c = sinComentarios[i];
    if (c === "{") {
      if (depth === 0) {
        bodyStart = i + 1;
      }
      depth++;
    } else if (c === "}") {
      depth--;
      if (depth === 0) {
        out.push({
          selector: sinComentarios.slice(selStart, i).trim(),
          body: sinComentarios.slice(bodyStart, i),
        });
      }
      if (depth === 0) selStart = i + 1;
    }
  }
  return out;
}

/** `--nombre: valor` de un cuerpo de bloque. */
function tokensOf(body: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const m of body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    const [, nombre, valor] = m;
    if (nombre !== undefined && valor !== undefined) {
      map.set(nombre, valor.trim());
    }
  }
  return map;
}

/** Un valor cuenta como color si trae un color literal. `var(...)` no
 *  cuenta: un token que apunta a otro token se revisa por el que apunta. */
const ES_COLOR =
  /#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(|oklch\(|oklab\(|color-mix\(|\b(white|black|transparent|currentcolor)\b/;

const css = readFileSync(CSS_PATH, "utf8");
const bloques = topLevelBlocks(css);

const esBase = (sel: string) =>
  sel.split(",").some((s) => {
    const t = s.trim();
    return t === ":root" || t.includes('data-theme="dark"');
  });
const esClaro = (sel: string) => sel.split(",").some((s) => s.includes('data-theme="light"'));

const base = new Map<string, string>();
const claro = new Map<string, string>();
for (const b of bloques) {
  if (esBase(b.selector)) for (const [k, v] of tokensOf(b.body)) base.set(k, v);
  if (esClaro(b.selector)) for (const [k, v] of tokensOf(b.body)) claro.set(k, v);
}

/**
 * Tokens de color que el tema claro toma tal cual del base a proposito.
 * Cada uno lleva el motivo, para que ampliar la lista sea una decision y
 * no un descuido.
 */
const COMPARTIDOS_A_PROPOSITO = new Map<string, string>([
  [
    "--shadow-card",
    "Sombra negra con alpha. Funciona sobre superficie clara: no aparece un color oscuro plano, que es el sintoma que se busca aca.",
  ],
  [
    "--shadow-card-hover",
    "Mismo caso que --shadow-card.",
  ],
]);

describe("tokens.css: paridad entre tema oscuro y claro", () => {
  it("el parser encuentra los dos bloques de tema", () => {
    expect(base.size).toBeGreaterThan(40);
    expect(claro.size).toBeGreaterThan(30);
    // `--surface-deep` es el token que arranco este test.
    expect(base.has("--surface-deep")).toBe(true);
    expect(claro.has("--surface-deep")).toBe(true);
  });

  it("todo token de color del tema base tiene contraparte en claro", () => {
    const sinOverride: string[] = [];
    for (const [nombre, valor] of base) {
      if (!ES_COLOR.test(valor)) continue;
      if (claro.has(nombre)) continue;
      if (COMPARTIDOS_A_PROPOSITO.has(nombre)) continue;
      sinOverride.push(`${nombre}: ${valor}`);
    }
    expect(
      sinOverride,
      `tokens de color usados en var() que no tienen valor en [data-theme="light"].\n` +
        `En claro caen a transparente y la regla se aplica sin fondo.\n` +
        `Si el sharing es intencional, agregalos a COMPARTIDOS_A_PROPOSITO con el motivo.`,
    ).toEqual([]);
  });

  it("el valor claro de --surface-deep sits bien en la rampa del tema claro", () => {
    // El token queda entre --surface y --bg-elevated: el contenedor hundido
    // esta arriba de la superficie y los items elevated popean sobre el.
    // En claro eso es --surface #ffffff -> --surface-deep -> --bg-elevated
    // #f5f5f7, o sea el deep es el mas oscuro de los tres. En oscuro el
    // orden se da vuelta (surface #0a0a0a -> deep #131313 -> elevated
    // #1a1a1a) porque "elevated" significa mas claro sobre fondo oscuro.
    const superficie = claro.get("--surface")!;
    const elevado = claro.get("--bg-elevated")!;
    const deep = claro.get("--surface-deep")!;
    const lum = (hex: string) => parseInt(hex.replace("#", "").slice(0, 2), 16);
    expect(deep).toMatch(/^#[0-9a-fA-F]{6}$/);
    expect(lum(deep)).toBeLessThan(lum(superficie));
    expect(lum(deep)).toBeGreaterThan(lum(elevado));
  });
});
