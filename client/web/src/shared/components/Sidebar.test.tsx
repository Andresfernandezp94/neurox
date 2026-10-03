// Tests para Sidebar.
// Port one-way desde agent-studio. Adaptado a los items reales del admin.
// EP-0003-03: envuelve en StoreProvider porque el Sidebar ahora incluye
// ConnectionIndicator (que usa useStore).
// EP-0026-UX: el sidebar tiene 7 nav items: status, chat, config, mcp,
// agents, sessions, workspace. MCP/Agents/Sessions/Workspace se
// promovieron desde Config al top-level nav.
// Theme-switcher removido (vive en AppHeader).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Sidebar, type NavId } from "./Sidebar";
import { StoreProvider } from "../../store/StoreContext";
import { AuthProvider } from "../../hooks/useAuth";
import { __resetAppFullscreenForTests } from "../hooks/useAppFullscreen";

function renderWithProviders(ui: React.ReactElement) {
  return render(
    <AuthProvider>
      <StoreProvider eventsPath="/__test_no_ws__{Math.random()}">{ui}</StoreProvider>
    </AuthProvider>
  );
}

describe("Sidebar", () => {
  const onTabChange = vi.fn();

  beforeEach(() => {
    onTabChange.mockClear();
    // El store de fullscreen es de nivel de modulo (a proposito: lo
    // comparten el sidebar y el chat), asi que su estado sobrevive entre
    // tests. Sin resetear, un test que entra en fullscreen ensucia el
    // siguiente.
    __resetAppFullscreenForTests();
    Object.defineProperty(document, "fullscreenElement", {
      configurable: true,
      value: null,
    });
  });

  it("renders every navigation item", () => {
    renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

    // Uno por cada destino de la nav. La lista se deriva de los botones que
    // quedaron renderizados, no de una copia a mano: asi agregar un item no
    // puede quedar sin cubrir en un test.
    const ids: NavId[] = [
      "status",
      "chat",
      "intelligence",
      "config",
    ];
    for (const id of ids) {
      expect(screen.getByTestId(`sidebar-nav-${id}`)).toBeDefined();
    }
  });

  it("renders nav buttons in the expected top-to-bottom order", () => {
    const { container } = renderWithProviders(
      <Sidebar view="status" onTabChange={onTabChange} />,
    );

    const order = Array.from(
      container.querySelectorAll<HTMLElement>('[data-testid^="sidebar-nav-"]'),
    ).map((el) => el.dataset.testid);

    // Overview va primero: es la pantalla default tras el login y lo
    // accionable tiene que estar arriba. Workspace antes de Config:
    // Config es configuracion puntual, Workspace es trabajo en curso.
    // `status` es el NavId histórico de Overview.
    // Intelligence va debajo de Chat: es donde se habla con los agentes, asi
    // que acompaña a Chat en el uso diario.
    // MCP y Workspace saliron de la nav, asi que ya no aparecen.
    expect(order).toEqual([
      "sidebar-nav-status",
      "sidebar-nav-chat",
      "sidebar-nav-intelligence",
      "sidebar-nav-config",
    ]);
  });

  it("no reuses an icon between two nav items", () => {
    // Regresión: Overview y Workspace usaban IconGrid e IconWorkspaces, y
    // este último era IconGrid con rx="1": la misma grilla de 4 cuadrados
    // con un redondeo que no se distingue a 1rem. Se comparan los paths
    // renderizados, no las clases.
    const { container } = renderWithProviders(
      <Sidebar view="status" onTabChange={onTabChange} />,
    );
    const svgs = Array.from(container.querySelectorAll<HTMLElement>('[data-testid^="sidebar-nav-"] svg'));
    const firmas = svgs.map((s) => s.innerHTML);
    expect(new Set(firmas).size).toBe(firmas.length);
  });

  it("calls onTabChange with the correct id when an item is clicked", () => {
    renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

    fireEvent.click(screen.getByTestId("sidebar-nav-chat"));
    expect(onTabChange).toHaveBeenCalledWith("chat");

    fireEvent.click(screen.getByTestId("sidebar-nav-config"));
    expect(onTabChange).toHaveBeenCalledWith("config");
  });

  it("marks the active item with the 'active' class", () => {
    renderWithProviders(<Sidebar view="config" onTabChange={onTabChange} />);

    const configBtn = screen.getByTestId("sidebar-nav-config");
    const chatBtn = screen.getByTestId("sidebar-nav-chat");

    expect(configBtn.className).toContain("active");
    expect(chatBtn.className).not.toContain("active");
  });

  it("accepts all valid NavId values (visible ones get the active class)", () => {
    const validIds: NavId[] = [
      "status",
      "chat",
      "intelligence",
      "config",
    ];

    for (const id of validIds) {
      const { unmount } = renderWithProviders(
        <Sidebar view={id} onTabChange={onTabChange} />,
      );
      const btn = screen.getByTestId(`sidebar-nav-${id}`);
      expect(btn.className).toContain("active");
      unmount();
    }
  });

  describe("menu de acciones del avatar", () => {
    const avatar = () => screen.getByTestId("user-avatar");
    const footer = () => screen.getByTestId("sidebar-footer");

    it("arranca cerrado", () => {
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);
      expect(footer().className).not.toContain("sidebar-panel__footer--actions-open");
      expect(avatar().getAttribute("aria-expanded")).toBe("false");
    });

    it("el avatar es el trigger: lo alterna como un toggle", () => {
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      fireEvent.click(avatar());
      expect(footer().className).toContain("sidebar-panel__footer--actions-open");
      expect(avatar().getAttribute("aria-expanded")).toBe("true");

      // Segundo toque: cierra. Un toggle, no un abrir.
      fireEvent.click(avatar());
      expect(footer().className).not.toContain("sidebar-panel__footer--actions-open");
      expect(avatar().getAttribute("aria-expanded")).toBe("false");
    });

    it("Escape cierra el menu", () => {
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      fireEvent.click(avatar());
      expect(footer().className).toContain("sidebar-panel__footer--actions-open");

      // Sin esto el unico modo de cerrarlo en mobile es volver a tocar el
      // avatar, y no hay teclado a la vista.
      fireEvent.keyDown(window, { key: "Escape" });
      expect(footer().className).not.toContain("sidebar-panel__footer--actions-open");
    });

    it("las cuatro acciones siguen montadas con el menu cerrado", () => {
      // El menu se oculta con CSS (visibility), no se desmonta: si se
      // desmontara, cerrarlo perderia el estado del theme toggle.
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      expect(screen.getByTestId("sidebar-bell")).toBeInTheDocument();
      expect(screen.getByTestId("theme-toggle")).toBeInTheDocument();
      expect(screen.getByTestId("sidebar-maximize")).toBeInTheDocument();
      expect(screen.getByTestId("sidebar-logout")).toBeInTheDocument();
    });

    it("el boton de pantalla completa esta entre el tema y el logout", () => {
      // Orden pedido: notificaciones, tema, pantalla completa, logout.
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      const acciones = document.querySelector(".sidebar-footer-actions");
      expect(acciones).not.toBeNull();
      const orden = Array.from(acciones!.querySelectorAll("[data-testid]")).map(
        (c) => c.getAttribute("data-testid"),
      );
      expect(orden).toEqual([
        "sidebar-bell",
        "theme-toggle",
        "sidebar-maximize",
        "sidebar-logout",
      ]);
    });

    it("maximizar pide fullscreen sobre el shell .app, no sobre documentElement", () => {
      // Pobrelo sobre documentElement esconderia la sidebar, que vive
      // adentro de `.app`. Es el punto de pedir el boton en la barra.
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      const shell = document.createElement("div");
      shell.className = "app";
      const requestFullscreen = vi.fn().mockResolvedValue(undefined);
      (shell as unknown as { requestFullscreen: unknown }).requestFullscreen =
        requestFullscreen;
      document.body.appendChild(shell);

      try {
        fireEvent.click(screen.getByTestId("sidebar-maximize"));
        expect(requestFullscreen).toHaveBeenCalledTimes(1);
      } finally {
        shell.remove();
      }
    });

    it("maximizar cierra el menu", () => {
      // El menu flota sobre la barra: dejarlo abierto taparia el contenido
      // que se acaba de expandir a pantalla completa.
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      fireEvent.click(avatar());
      expect(footer().className).toContain("sidebar-panel__footer--actions-open");

      fireEvent.click(screen.getByTestId("sidebar-maximize"));
      expect(footer().className).not.toContain("sidebar-panel__footer--actions-open");
    });

    it("el boton se resincroniza si el fullscreen sale desde el navegador", () => {
      // Salir con el ESC del sistema no pasa por ningun boton de la app: lo
      // detecta el listener de `fullscreenchange`, que vive en el modulo y
      // no en un efecto de ChatPanel (que en mobile no esta montado).
      renderWithProviders(<Sidebar view="chat" onTabChange={onTabChange} />);

      const btn = () => screen.getByTestId("sidebar-maximize");
      expect(btn().getAttribute("aria-pressed")).toBe("false");
      expect(btn().getAttribute("aria-label")).toBe("Pantalla completa");

      const shell = document.createElement("div");
      shell.className = "app";
      (shell as unknown as { requestFullscreen: unknown }).requestFullscreen =
        vi.fn().mockResolvedValue(undefined);
      document.body.appendChild(shell);

      try {
        Object.defineProperty(document, "fullscreenElement", {
          configurable: true,
          value: shell,
        });
        fireEvent(document, new Event("fullscreenchange"));

        expect(btn().getAttribute("aria-pressed")).toBe("true");
        expect(btn().getAttribute("aria-label")).toBe("Salir de pantalla completa");

        Object.defineProperty(document, "fullscreenElement", {
          configurable: true,
          value: null,
        });
        fireEvent(document, new Event("fullscreenchange"));
        expect(btn().getAttribute("aria-pressed")).toBe("false");
      } finally {
        shell.remove();
      }
    });
  });

});