import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { reglasEnMediaQuery, esDisplayNone } from "./media-query";

/**
 * En mobile el bloque de usuario del sidebar queda en modo avatar: solo el
 * icono con su halo de estado. Las cuatro acciones (campana, tema, pantalla
 * completa, logout) no ocupan lugar en la barra: aparecen al tocar el avatar
 * como una columna que crece hacia arriba, con cada icono flotando suelto.
 *
 * Los bugs que estos guards fijan, en orden:
 *
 *  1. El breakpoint no decia nada del `.user-card`, asi que conservaba el
 *     layout de desktop (avatar, nombre y rol en fila) dentro de una barra
 *     de 3.5rem. El texto se comia el ancho de los iconos.
 *
 *  2. Con la columna flotando hacia ARRIBA, el `.sidebar-panel` con
 *     `overflow: hidden` la recortaba: se veia un borde asomando. Por eso en
 *     mobile el panel va `overflow: visible`, y el nav conserva su propio
 *     `overflow: hidden`.
 *
 *  3. El margen que reserva el avatar para las fichas usaba `var()` sin
 *     fallback. Un `var()` no definido no da error: invalida la declaracion
 *     y deja el margen en 0, que es el menu encima del avatar. Con fallback,
 *     una variable que falte degrada a la medida correcta.
 *
 *  4. El avatar no puede marcarse con un cuadro redondeado al presionarlo:
 *     en mobile eso se lee como un quinto boton rectangular al lado de los
 *     cuatro que flotan sueltos.
 *
 * Ojo con la direccion de la regla collapsed: `.sidebar-panel.collapsed`
 * esconde `.user-info` Y `.sidebar-footer-actions`, pero ese camino no aplica
 * en mobile porque el componente fuerza `collapsed = false` cuando
 * `isMobile()`. Asi que en mobile no hay que tocar la regla collapsed.
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

  it("mobile conserva las cuatro acciones montadas", () => {
    // Se ocultan con `visibility`, no se desmontan: si se desmontaran,
    // cerrarlas perderia el estado del theme toggle.
    expect(esDisplayNone(mobile.get(".sidebar-footer-actions"))).toBe(false);
    expect(sidebar).toMatch(/className="sidebar-footer-action sidebar-bell"/);
    expect(sidebar).toMatch(/<ThemeToggle className="sidebar-footer-action"/);
    expect(sidebar).toMatch(/className="sidebar-maximize sidebar-footer-action"/);
    expect(sidebar).toMatch(/className="sidebar-logout sidebar-footer-action"/);
  });

  describe("la columna de acciones", () => {
    it("crece hacia arriba desde el avatar", () => {
      // `column-reverse`: en el DOM las acciones van campana, tema,
      // pantalla completa, logout; en pantalla se leen de abajo hacia
      // arriba, con la campana pegada al avatar.
      const acciones = mobile.get(".sidebar-footer-actions");
      expect(acciones).toMatch(/flex-direction:\s*column-reverse/);
      expect(acciones).toMatch(/bottom:\s*100%/);
      // Fuera del flujo: en el flujo normal la barra sumaria las filas de
      // alto y los nav-btn se correrian.
      expect(acciones).toMatch(/position:\s*absolute/);
    });

    it("cada icono es una ficha independiente", () => {
      // Sin contenedor comun: fondo, borde y sombra por ficha, para que la
      // columna se lea como cuatro botones y no como una barra.
      const acciones = mobile.get(".sidebar-footer-actions");
      expect(acciones).not.toMatch(/box-shadow:\s*var\(/);
      expect(acciones).not.toMatch(/border:\s*1px solid/);

      const ficha = mobile.get(".sidebar-footer-actions .sidebar-footer-action");
      expect(ficha).toMatch(/background:\s*var\(/);
      expect(ficha).toMatch(/border:\s*1px solid/);
      expect(ficha).toMatch(/box-shadow:\s*var\(/);
    });

    it("las fichas se centran sobre la linea del avatar", () => {
      // El borde derecho del avatar coincide con el borde del contenido del
      // footer, asi que el margen es SOLO el padding del pie. Sumar tambien
      // el padding de la tarjeta (como se hizo primero) lo corria 0.7rem a
      // la izquierda.
      const acciones = mobile.get(".sidebar-footer-actions");
      expect(acciones).toMatch(/right:\s*var\(--sidebar-foot-pad-x/);
      expect(acciones).not.toMatch(/right:\s*calc\([^)]*user-card-pad-x/);

      // El ancho fijo del avatar + `align-items: center` hacen que las fichas
      // se centren solas, sin sumar medidas a mano.
      expect(acciones).toMatch(/width:\s*var\(--user-avatar-size/);
      expect(acciones).toMatch(/align-items:\s*center/);
    });

    it("las medidas que se suman tienen fallback", () => {
      // Un `var()` sin fallback invalida la declaracion entera y deja el
      // margen en 0: el menu queda encima del avatar. Ese fue el bug.
      const acciones = mobile.get(".sidebar-footer-actions");
      for (const uso of acciones!.matchAll(/var\((--[a-z-]+)([^)]*)\)/g)) {
        expect(uso[2], `var(${uso[1]}) necesita fallback`).toContain(",");
      }
    });

    it("cerrado queda invisible y fuera del tab order", () => {
      // Solo con `opacity` las fichas quedarian invisibles pero alcanzables
      // con el teclado.
      const cerrado = mobile.get(
        ".sidebar-panel__footer:not(.sidebar-panel__footer--actions-open) .sidebar-footer-actions .sidebar-footer-action",
      );
      expect(cerrado).toBeDefined();
      expect(cerrado).toMatch(/visibility:\s*hidden/);
      expect(cerrado).toMatch(/pointer-events:\s*none/);
    });
  });

  it("el panel no recorta la columna en mobile", () => {
    // La base es `overflow: hidden`: recorta todo lo que salga de la barra,
    // que es donde se veia el menu cortado asomando arriba.
    const panel = mobile.get(".sidebar-panel");
    expect(panel).toBeDefined();
    expect(panel).not.toMatch(/overflow:\s*hidden/);

    // El nav conserva su propio recorte: es lo unico que puede desbordar en
    // horizontal y es lo que evita que la barra se desborde.
    const nav = mobile.get(".sidebar-panel__nav");
    expect(nav).toMatch(/overflow:\s*hidden/);
  });

  it("el avatar no se marca con un cuadro al presionarlo ni al hover", () => {
    // La base tiene `.user-menu:hover .user-card { background:
    // var(--bg-elevated) }`. En mobile eso encierra al avatar en un
    // rectangulo redondeado, al lado de cuatro fichas que flotan sueltas: se
    // lee como un quinto boton.
    const base = css.match(/\.user-menu:hover \.user-card\s*\{([^}]*)\}/);
    expect(base).not.toBeNull();
    expect(base![1]).toMatch(/background:\s*var\(--bg-elevated\)/);

    for (const sel of [
      ".user-menu:hover .user-card",
      ".sidebar-panel__footer--actions-open .user-card",
    ]) {
      const cuerpo = mobile.get(sel);
      expect(cuerpo, `falta la regla mobile de ${sel}`).toBeDefined();
      expect(cuerpo).toMatch(/background:\s*transparent/);
    }
  });

  it("el markup sigue renderizando la info y el tooltip", () => {
    // El CSS es una cosa y el markup otra: si un cambio futuro sacara el
    // nombre del HTML, los guards de CSS de arriba seguirian pasando.
    const user = readFileSync(join(aqui, "../../components/UserPlaceholder.tsx"), "utf8");
    expect(user).toMatch(/className="user-info"/);
    expect(user).toMatch(/user-info__name/);
    expect(user).toMatch(/user-info__role/);
    // El nombre y el rol siguen en el tooltip: en mobile deja de ocupar
    // pantalla, pero no se pierde.
    expect(user).toMatch(/title=\{user \? `\$\{user\.username\}/);
  });

  it("el avatar es el trigger del menu y anuncia su estado", () => {
    expect(sidebar).toMatch(/onTriggerClick=\{\(\) => setActionsOpen\(\(o\) => !o\)\}/);
    expect(sidebar).toMatch(/actionsOpen=\{actionsOpen\}/);
    // Sin `aria-expanded`, un lector de pantalla no distingue "avatar" de
    // "avatar con el menu abierto".
    const user = readFileSync(join(aqui, "../../components/UserPlaceholder.tsx"), "utf8");
    expect(user).toMatch(/aria-expanded=\{actionsOpen === undefined \? undefined : actionsOpen\}/);
  });

  it("el panel nunca llega a `collapsed` en mobile", () => {
    // Es lo que hace que la regla `.sidebar-panel.collapsed` (que esconde
    // tambien las acciones) no aplique en mobile. Si esto cambia, los
    // iconos desaparecen y un guard de CSS solo no lo avisaria.
    expect(sidebar).toMatch(/const collapsed = isMobile\(\) \? false : !expanded/);
  });
});
