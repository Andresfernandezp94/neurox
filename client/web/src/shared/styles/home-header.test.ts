import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/**
 * El header de la landing tiene que conservar sus controles accionables en
 * mobile.
 *
 * El bug: en el breakpoint de 640px la regla era `.home__nav { display:
 * none }`. `.home__nav` es el grupo entero, no los anchors: adentro estan
 * los links de seccion, el theme toggle y el boton "Entrar". Escondiendo
 * el grupo, en mobile no quedaba forma de entrar al panel ni de cambiar de
 * tema, que es justo lo unico accionable en una pantalla chica.
 *
 * Se chequea sobre la hoja de estilos porque `vitest.config.ts` tiene
 * `css: false`: el CSS ni se procesa y ningun test de componente puede
 * observar que algo se escondio por media query.
 */

const CSS_PATH = join(dirname(fileURLToPath(import.meta.url)), "home.css");
const css = readFileSync(CSS_PATH, "utf8");

/** Reglas declaradas dentro de un breakpoint, por selector. */
function reglasEnMediaQuery(maxWidth: number): Map<string, string> {
  const out = new Map<string, string>();
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

  for (const m of css.slice(ini, i).matchAll(/(^|\n)\s*(\.[a-zA-Z0-9_-]+)\s*\{([^}]*)\}/g)) {
    const selector = m[2];
    const body = m[3];
    if (selector !== undefined && body !== undefined) out.set(selector, body);
  }
  return out;
}

describe("landing: header usable en mobile", () => {
  const mobile = reglasEnMediaQuery(640);

  it("el breakpoint de mobile existe (si no, el selector cambio de nombre)", () => {
    expect(mobile.size).toBeGreaterThan(3);
  });

  it("mobile no esconde el grupo entero del nav", () => {
    // La regresion: `.home__nav` con display:none se llevaba tambien el
    // theme toggle y el boton Entrar.
    const nav = mobile.get(".home__nav");
    expect(
      nav === undefined || !/display:\s*none/.test(nav),
      ".home__nav no puede quedar con display:none en mobile: adentro estan " +
        "el theme toggle y el boton Entrar, no solo los anchors.",
    ).toBe(true);
  });

  it("mobile esconde solo los anchors de seccion", () => {
    // Los anchors apuntan a secciones de la misma pagina, que en mobile se
    // llegan scrolleando: ocultarlos es la decision correcta.
    const links = mobile.get(".home__nav-links");
    expect(links).toBeDefined();
    expect(links).toMatch(/display:\s*none/);
  });

  it("el header mantiene sus tres grupos en el markup", () => {
    // El CSS es una cosa y el markup otra: si el header dejara de renderizar
    // el boton Entrar, el guard de CSS seguiria pasando.
    const home = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), "../../components/HomePage.tsx"),
      "utf8",
    );
    expect(home).toMatch(/className="home__nav-links"/);
    expect(home).toMatch(/home__nav-theme/);
    expect(home).toMatch(/data-testid="home-enter"/);
  });
});