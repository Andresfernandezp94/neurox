import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/**
 * Los badges no pueden dejar las esquinas vivas.
 *
 * `.badge--active` (el pill accent solido que marca lo activo: el provider
 * en ProvidersPanel y en el Overview, el modelo en ModelsTab, el rol en
 * UsersPanel, el env en EnvTab) tenia `border-radius: 0` mientras `.badge`
 * usa `--radius-sm`. Con eso el pill se leia como una etiqueta pegada y los
 * badges "iguales" de la app se veian distintos entre si.
 *
 * Se chequea sobre la hoja de estilos y no sobre un componente porque el
 * radio no se puede observar desde vitest: la config tiene `css: false`, asi
 * que el CSS ni se procesa y `getComputedStyle` no ve nada. Un test de
 * componente que verifique el radio pasaria siempre.
 */
const CSS_PATH = join(dirname(fileURLToPath(import.meta.url)), "atoms.css");

function reglasBadge(css: string): { selector: string; body: string }[] {
  const out: { selector: string; body: string }[] = [];
  for (const m of css.matchAll(/^\.(badge[a-z0-9_-]*)\s*\{([^}]*)\}/gm)) {
    const selector = m[1];
    const body = m[2];
    if (selector !== undefined && body !== undefined) {
      out.push({ selector, body });
    }
  }
  return out;
}

describe("badges: esquinas redondeadas", () => {
  const reglas = reglasBadge(readFileSync(CSS_PATH, "utf8"));

  it("encuentra las reglas de badge (si no encuentra, el selector cambio)", () => {
    expect(reglas.length).toBeGreaterThan(4);
    expect(reglas.some((r) => r.selector === "badge")).toBe(true);
    expect(reglas.some((r) => r.selector === "badge--active")).toBe(true);
  });

  it("el pill activo no deja las esquinas vivas", () => {
    const activo = reglas.find((r) => r.selector === "badge--active")!;
    expect(activo.body).not.toMatch(/border-radius:\s*0(?:[;\s]|$)/);
  });

  it("el pill activo toma su radio de un token, no de un numero", () => {
    // Que sea un token y no un literal: asi un cambio de escala se propaga
    // a los seis usos del pill de una.
    const activo = reglas.find((r) => r.selector === "badge--active")!;
    expect(activo.body).toMatch(/border-radius:\s*var\(--radius-/);
  });

});