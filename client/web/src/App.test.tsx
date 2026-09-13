import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, it, expect, vi } from "vitest";
import App from "./App";

// Estado del mock hoisted: permite que cada test reconfigure `auth_required`
// (gate de LoginScreen) sin necesidad de re-mockear el módulo entero.
const mockState = vi.hoisted(() => ({
  authRequired: false,
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
        sessions: [],
        approvals: [],
        events: [],
        loaded: { agents: false, sessions: false, approvals: false, health: true },
      },
      dispatch: () => {},
      snapshot: vi.fn(),
    }),
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
}));

describe("App", () => {
  beforeEach(() => {
    window.localStorage.clear();
    sessionStorage.clear();
    mockState.authRequired = false;
  });

  afterEach(() => {
    window.localStorage.clear();
    sessionStorage.clear();
    vi.restoreAllMocks();
    // Limpia el query string para no contaminar otros tests.
    const url = new URL(window.location.href);
    url.search = "";
    window.history.replaceState({}, "", url.toString());
  });

  it("renders without crashing", () => {
    const { container } = render(<App />);
    expect(container.querySelector(".app")).toBeInTheDocument();
  });

  it("renders Sidebar with navigation items", () => {
    render(<App />);
    expect(screen.getByTestId("sidebar-nav-chat")).toBeInTheDocument();
    expect(screen.getByTestId("sidebar-nav-config")).toBeInTheDocument();
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

    render(<App />);
    expect(screen.queryByTestId("app-header")).not.toBeInTheDocument();
  });

  it("hides the AppHeader when ?hideHeader=1 is in the URL", () => {
    const url = new URL(window.location.href);
    url.search = "?hideHeader=1";
    window.history.replaceState({}, "", url.toString());

    render(<App />);
    expect(screen.queryByTestId("app-header")).not.toBeInTheDocument();
  });

  it("renders LoginScreen when auth_required is true and there's no token", () => {
    // Regresión para "Rendered fewer hooks than expected": useHideHeader
    // debe ejecutarse antes del early return de LoginScreen. Si no lo
    // hace, React tira el error porque el conteo de hooks cambia entre
    // renders (auth_required=true vs false).
    mockState.authRequired = true;

    // Suprime el error boundary que React dispararía si la regla de
    // hooks se rompe. Si el error aparece, el test falla.
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    render(<App />);

    // No debe haber AppHeader (estamos en LoginScreen, fuera del shell).
    expect(screen.queryByTestId("app-header")).not.toBeInTheDocument();
    // El botón submit de LoginScreen debe estar presente (form de auth).
    expect(screen.getByTestId("login-submit")).toBeInTheDocument();

    // Si hubo violación de Rules of Hooks, React loggea el error a
    // console.error. Verificamos que no se haya disparado.
    const hookErrors = consoleErrorSpy.mock.calls.filter((args) =>
      String(args[0] ?? "").includes("Rendered fewer hooks"),
    );
    expect(hookErrors).toHaveLength(0);

    consoleErrorSpy.mockRestore();
  });
});