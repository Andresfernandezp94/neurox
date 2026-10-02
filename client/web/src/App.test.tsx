import { act, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, it, expect, vi } from "vitest";
import App from "./App";

// Estado del mock hoisted: permite que cada test reconfigure `auth_required`
// (gate de LoginScreen) sin necesidad de re-mockear el módulo entero.
// `dispatch` también va hoisted: la factory de vi.mock se evalúa antes
// que el cuerpo del módulo, así que referenciar una `const` de abajo
// daría ReferenceError (TDZ).
const mockState = vi.hoisted(() => ({
  authRequired: false,
  dispatch: vi.fn(),
}));

// Mock the store context to avoid real WS connections.
// EP-0024: useConnectionState ahora devuelve ConnectionState real
// (ws/health/latencyMs/lastPongAt/isZombie/retriesTotal/lastRetryAt),
// no la forma vieja con wsStatus.
vi.mock("./store/StoreContext", () => {
  return {
    StoreProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
    useStore: () => ({
      state: {
        connection: {
          health: {
            service: "neurox",
            version: "0.4.0",
            uptime: 123,
            get auth_required() {
              return mockState.authRequired;
            },
          },
        },
        agents: new Map(),
        sessions: new Map(),
        approvals: new Map(),
        events: [],
        loaded: { agents: false, sessions: false, approvals: false, health: true },
      },
      dispatch: mockState.dispatch,
      snapshot: vi.fn(),
    }),
    useStoreDispatch: () => mockState.dispatch,
    useConnectionState: () => ({
      ws: "open" as const,
      health: {
        service: "neurox",
        version: "0.4.0",
        uptime_seconds: 123,
        status: "ok",
      },
      latencyMs: 12,
      lastPongAt: Date.now(),
      isZombie: false,
      retriesTotal: 0,
      lastRetryAt: null,
    }),
    initialState: {},
    reducer: (s: unknown) => s,
  };
});

// Mock the API calls that panels might do on mount
vi.mock("./api/agents", () => ({
  startAgent: vi.fn(),
  stopAgent: vi.fn(),
}))

// El BootGate espera `minMs` aunque el store ya esté listo (para que el
// loader no parpadee). Con timers reales de jsdom eso nunca avanza dentro
// del test, así que estos tests exercisean el shell con el gate abierto.
vi.mock("./shared/mock/mockData", () => ({
  isMockMode: () => true,
  mockSnapshots: () => ({
    health: { service: "neurox", status: "ok", version: "0.4.0-test" },
    agents: [],
    sessions: [],
    approvals: [],
    latencyMs: 1,
  }),
}))

describe("App", () => {
  // El click dispara un state update en `Landing` (entered: true), así que
  // va envuelto en act() para que React lo procese antes del assert.
  const click = (testId: string) =>
    act(() => {
      screen.getByTestId(testId).click();
    });

  beforeEach(() => {
    window.localStorage.clear();
    sessionStorage.clear();
    mockState.authRequired = false;
    // Cada test arranca en la landing: la ruta vive en la URL, así que
    // hay que resetearla explícitamente.
    window.history.replaceState({}, "", "/");
  });

  afterEach(() => {
    window.localStorage.clear();
    sessionStorage.clear();
    vi.restoreAllMocks();
    window.history.replaceState({}, "", "/");
  });

  it("shows the landing at /", () => {
    const { container } = render(<App />);
    expect(screen.getByTestId("home")).toBeInTheDocument();
    expect(container.querySelector(".app")).not.toBeInTheDocument();
    expect(screen.queryByTestId("login-submit")).not.toBeInTheDocument();
  });

  it("navigates to /login from the landing entry button", () => {
    render(<App />);
    click("home-enter");
    expect(window.location.pathname).toBe("/login");
  });

  it("shows the login screen at /login directly", () => {
    // Deep link: /login tiene que renderizar el login sin pasar por la
    // landing. Antes esto era imposible (todo vivía en la misma URL).
    window.history.replaceState({}, "", "/login");
    render(<App />);
    expect(screen.getByTestId("login-submit")).toBeInTheDocument();
    expect(screen.queryByTestId("home")).not.toBeInTheDocument();
  });

  it("returns to the landing from /login via the back button", () => {
    window.history.replaceState({}, "", "/login");
    render(<App />);
    click("login-back");
    expect(window.location.pathname).toBe("/");
    expect(screen.getByTestId("home")).toBeInTheDocument();
  });

  it("shows the login when deep-linking to /app without a session", () => {
    // Guarda: /app sin token no debe dejar el panel a medias pidiendo
    // datos que van a fallar con 401.
    mockState.authRequired = true;
    window.history.replaceState({}, "", "/app");
    render(<App />);
    expect(screen.getByTestId("login-submit")).toBeInTheDocument();
  });

  it("falls back to the landing for an unknown route", () => {
    window.history.replaceState({}, "", "/no-existe");
    render(<App />);
    expect(screen.getByTestId("home")).toBeInTheDocument();
  });

  it("renders the admin shell at /app when auth is not required", async () => {
    window.history.replaceState({}, "", "/app");
    render(<App />);
    await screen.findByTestId("sidebar-nav-chat");
    expect(screen.getByTestId("sidebar-nav-config")).toBeInTheDocument();
    expect(screen.getByTestId("status-panel")).toBeInTheDocument();
  });

  it("monta el panel de Intelligence y sus tabs de agentes", async () => {
    // Que el boton de la nav exista no dice nada de que el panel monte: el
    // render por tab en App.tsx es un `activeTab === "..."` mas, y un id
    // agregado a la nav sin su bloque de render deja la pantalla en blanco.
    window.localStorage.setItem("active-tab", "intelligence");
    window.history.replaceState({}, "", "/app");
    render(<App />);

    // El panel existe y trae el AgentsPanel adentro, no otra cosa.
    const panel = await screen.findByTestId("intelligence-panel");
    expect(panel).toBeInTheDocument();
    expect(within(panel).getByTestId("agents-tab-agents")).toBeInTheDocument();
    expect(within(panel).getByTestId("agents-tab-skills")).toBeInTheDocument();
    expect(within(panel).getByTestId("agents-tab-tools")).toBeInTheDocument();

    // Y el item queda marcado como activo.
    expect(screen.getByTestId("sidebar-nav-intelligence").className).toContain("active");

    window.localStorage.removeItem("active-tab");
  });

  it("monta el panel de MCP desde la nav", async () => {
    // MCP paso a ser destino de la nav. El test del boton no alcanza: lo que
    // importa es que el panel monte.
    window.localStorage.setItem("active-tab", "mcp");
    window.history.replaceState({}, "", "/app");
    render(<App />);

    const panel = await screen.findByTestId("mcp-panel");
    expect(within(panel).getByTestId("config-mcp-tab")).toBeInTheDocument();
    expect(screen.getByTestId("sidebar-nav-mcp").className).toContain("active");

    window.localStorage.removeItem("active-tab");
  });

  it("offers a way back to the landing from the login", () => {
    mockState.authRequired = true;
    window.history.replaceState({}, "", "/login");
    render(<App />);
    expect(screen.getByTestId("login-back")).toBeInTheDocument();
  });

  it("does not render a separate AppHeader anymore", () => {
    // EP-0024/EP-0026-UX: el AppHeader fue absorbido por el shell y
    // eliminado de App.tsx; la UI ya no lo renderiza.
    render(<App />);
    expect(screen.queryByTestId("app-header")).not.toBeInTheDocument();
  });

  it("hides the AppHeader by default on a mobile viewport", () => {
    // Mock matchMedia as mobile so useHideHeader's viewport default
    // kicks in (per user request: "en mobile no se debe ver el header").
    vi.spyOn(window, "matchMedia").mockImplementation(
      (query: string) =>
        ({
          matches: query.includes("max-width: 1024"),
          media: query,
          onchange: null,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
          addListener: vi.fn(),
          removeListener: vi.fn(),
          dispatchEvent: vi.fn(),
        }) as unknown as MediaQueryList,
    );

    window.history.replaceState({}, "", "/app");
    render(<App />);
    expect(screen.queryByTestId("app-header")).not.toBeInTheDocument();
  });

  it("hides the AppHeader when ?hideHeader=1 is in the URL", () => {
    window.history.replaceState({}, "", "/app?hideHeader=1");

    render(<App />);
    expect(screen.queryByTestId("app-header")).not.toBeInTheDocument();
  });

  it("does not break the Rules of Hooks when auth_required is true", async () => {
    // Regresión para "Rendered fewer hooks than expected": los hooks del
    // Admin deben ejecutarse antes del early return de LoginScreen.
    // Antes esto se disparaba entrando directo al admin; ahora hay que
    // deep-linkear a /app sin sesión.
    mockState.authRequired = true;
    window.history.replaceState({}, "", "/app");

    // Suprime el error boundary que React dispararía si la regla de
    // hooks se rompe. Si el error aparece, el test falla.
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    render(<App />);
    await screen.findByTestId("login-submit");

    // Si hubo violación de Rules of Hooks, React loggea el error a
    // console.error. Verificamos que no se haya disparado.
    const hookErrors = consoleErrorSpy.mock.calls.filter((args) =>
      String(args[0] ?? "").includes("Rendered fewer hooks"),
    );
    expect(hookErrors).toHaveLength(0);

    consoleErrorSpy.mockRestore();
  });
});