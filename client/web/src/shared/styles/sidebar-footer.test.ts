import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { reglasEnMediaQuery, esDisplayNone } from "./media-query";

/**
 * En mobile el bloque de usuario del sidebar tiene que quedar en modo
 * avatar: solo el icono con su halo de estado mas los tres iconos de
 * sesion (campana, tema, logout).
 *
 * El bug: el breakpoint de 1024px no decia nada de `.user-card`, asi que
 * conservaba el layout de desktop -- avatar, nombre y rol en fila -- dentro
 * de una bottom-bar de 3.5rem de alto. El texto se comia el ancho que
 * necesitan los iconos y los dejaba apretados.
 *
 * Ojo con la direccion de la regla: `.sidebar-panel.collapsed` esconde
 * `.user-info` Y `.sidebar-footer-actions`, pero ese camino no aplica en
 * mobile porque el componente fuerza `collapsed = false` cuando
 * `isMobile()`. Asi que en mobile no hay que tocar la regla collapsed, hay
 * que ocultar solo `.user-info`.
 *
 * Se chequea sobre la hoja de estilos porque `vitest.config.ts` tiene
 * `css: false`: el CSS ni se procesa y ningun test de componente puede
 * observar que algo se escondio por media query.
 */

const aqui = dirname(fileURLToPath(import.meta.url));
const css = readFileSync(join(aqui, "sidebar.css"), "utf8");
const sidebar = readFileSync(join(aqui, "../components/Sidebar.tsx"), "utf8");

const mobile = reglasEnMediaQuery(css, 1024);

describe("sidebar: el bloque de usuario en mobile", () => {
  it("el breakpoint de mobile existe (si no, el selector cambio de nombre)", () => {
    expect(mobile.size).toBeGreaterThan(5);
  });

  it("mobile oculta nombre y rol", () => {
    const info = mobile.get(".user-info");
    expect(info).toBeDefined();
    expect(esDisplayNone(info)).toBe(true);
  });

  it("mobile conserva el avatar (es lo unico que identifica al usuario)", () => {
    // El avatar lleva el halo de estado de conexion (`.conn-avatar`): es el
    // indicador de salud de la sesion, no una decoracion.
    expect(esDisplayNone(mobile.get(".user-avatar"))).toBe(false);
    expect(esDisplayNone(mobile.get(".user-card"))).toBe(false);
  });

  it("mobile conserva los tres iconos de sesion", () => {
    // Campana, tema y logout son lo accionable del pie. Escondidos en
    // mobile no queda forma de cambiar de tema ni de cerrar sesion sin
    // subir al panel.
    expect(esDisplayNone(mobile.get(".sidebar-footer-actions"))).toBe(false);
  });

  it("el markup sigue renderizando la info y los tres iconos", () => {
    // El CSS es una cosa y el markup otra: si un cambio futuro sacara el
    // nombre del HTML, los guards de CSS de arriba seguirian pasando.
    expect(sidebar).toMatch(/className="sidebar-footer-actions"/);
    expect(sidebar).toMatch(/className="sidebar-footer-action sidebar-bell"/);
    expect(sidebar).toMatch(/className="sidebar-logout sidebar-footer-action"/);
    expect(sidebar).toMatch(/<ThemeToggle className="sidebar-footer-action"/);

    const user = readFileSync(join(aqui, "../../components/UserPlaceholder.tsx"), "utf8");
    expect(user).toMatch(/className="user-info"/);
    expect(user).toMatch(/user-info__name/);
    expect(user).toMatch(/user-info__role/);
    // El nombre y el rol siguen en el tooltip: en mobile deja de ocupar
    // pantalla, pero no se pierde.
    expect(user).toMatch(/title=\{user \? `\$\{user\.username\}/);
  });

  it("el panel nunca llega a `collapsed` en mobile", () => {
    // Es lo que hace que la regla `.sidebar-panel.collapsed` (que esconde
    // tambien los tres iconos) no aplique en mobile. Si esto cambia, los
    // iconos desaparecen y el guard de CSS no lo va a avisar.
    expect(sidebar).toMatch(/const collapsed = isMobile\(\) \? false : !expanded/);
  });
});
