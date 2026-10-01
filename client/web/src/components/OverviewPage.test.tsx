import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { OverviewPage } from "./OverviewPage";

const mockStore = vi.hoisted(() => ({
  agents: 3,
  sessions: 2,
  approvals: 1,
  ws: "open" as "open" | "closed" | "connecting",
  health: { service: "neurox", version: "0.4.0", status: "ok", uptime_seconds: 120 } as Record<string, unknown> | null,
  latencyMs: 14,
}));

const mockApi = vi.hoisted(() => ({
  providers: [] as unknown[],
  plugins: [] as unknown[],
  tools: [] as unknown[],
  toolsCalled: false,
  rejectAll: false,
}));

vi.mock("../api/llm", () => ({
  getProviders: () =>
    mockApi.rejectAll
      ? Promise.reject(new Error("boom"))
      : Promise.resolve({
          providers: mockApi.providers,
          default_provider: "minimax",
          default_model: "M3",
        }),
}));

// OJO: `getPlugins` devuelve el CATÁLOGO DE TOOLS, no servidores plugin.
// Ver api/mcps.ts — el shape real no tiene `status`, `tools` ni `base_url`.
vi.mock("../api/mcps", () => ({
  getPlugins: () =>
    mockApi.rejectAll
      ? Promise.reject(new Error("boom"))
      : Promise.resolve(mockApi.plugins),
}));

vi.mock("../api/tools", () => ({
  // El overview ya NO pide /v1/tools: el total de tools viene como hint
  // de la tarjeta de Categorías. El mock queda para detectar si alguien
  // vuelve a introducir ese request.
  listTools: () => {
    mockApi.toolsCalled = true;
    return Promise.resolve({ tools: mockApi.tools });
  },
}));

vi.mock("../store/StoreContext", () => ({
  useStore: () => ({
    state: {
      agents: new Map(Array.from({ length: mockStore.agents }, (_, i) => [i, {}])),
      sessions: new Map(Array.from({ length: mockStore.sessions }, (_, i) => [i, {}])),
      approvals: new Map(Array.from({ length: mockStore.approvals }, (_, i) => [i, {}])),
      connection: {
        ws: mockStore.ws,
        health: mockStore.health,
        latencyMs: mockStore.latencyMs,
        lastPongAt: Date.now(),
        isZombie: false,
        retriesTotal: 0,
        lastRetryAt: null,
      },
    },
  }),
  useConnectionState: () => ({
    ws: mockStore.ws,
    health: mockStore.health,
    latencyMs: mockStore.latencyMs,
    lastPongAt: Date.now(),
    isZombie: false,
    retriesTotal: 0,
    lastRetryAt: null,
  }),
}));

const makeTool = (over: Record<string, unknown> = {}) => ({
  name: "shell",
  description: "Ejecuta un comando",
  parameters: { type: "object", properties: {} },
  requires_approval: false,
  categories: ["system"],
  mode_compatible: ["build"],
  ...over,
});

describe("OverviewPage", () => {
  beforeEach(() => {
    mockStore.ws = "open";
    mockStore.health = {
      service: "neurox",
      version: "0.4.0",
      status: "ok",
      uptime_seconds: 120,
    };
    mockStore.latencyMs = 14;
    mockStore.agents = 3;
    mockStore.sessions = 2;
    mockStore.approvals = 1;
    mockApi.rejectAll = false;
    mockApi.toolsCalled = false;
    mockApi.providers = [];
    mockApi.plugins = [];
    mockApi.tools = [];
  });

  it("renders the overview", () => {
    render(<OverviewPage />);
    expect(screen.getByTestId("overview")).toBeInTheDocument();
  });

  it("renders the connection status with a label, not just a dot", () => {
    // Regresión de diseño: el indicador era un Badge con punto pulsante,
    // que es el recurso visual más genérico de "AI dashboard".
    render(<OverviewPage />);
    const el = screen.getByTestId("connection-status");
    expect(el).toHaveAttribute("data-kind", "live");
    expect(el).toHaveTextContent("en línea");
  });

  it("reads counts from the store, not from its own fetch", () => {
    // Regresión de diseño: agents/sessions/approvals salen del store
    // global (mantenido por el WS), no de un request propio.
    render(<OverviewPage />);
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
  });

  it("surfaces pending approvals as needing attention", async () => {
    render(<OverviewPage />);
    await waitFor(() =>
      expect(screen.getByText(/aprobación.*pendiente/i)).toBeInTheDocument(),
    );
  });

  it("warns when providers have no API key", async () => {
    mockApi.providers = [
      { id: "minimax", kind: "minimax", model: "M3", base_url: "", configured: false, active: false },
      { id: "mistral", kind: "openai_compat", model: "large", base_url: "", configured: true, active: true },
    ];
    render(<OverviewPage />);
    await waitFor(() =>
      expect(screen.getByText(/provider.*sin API key/i)).toBeInTheDocument(),
    );
  });

  it("renders the tool catalog grouped by category", async () => {
    // Regresión del bug reportado: antes se hacía `p.tools.length` sobre
    // /v1/mcps, que devuelve un array de tool specs sin esa propiedad.
    mockApi.plugins = [
      makeTool({ name: "shell", categories: ["system"] }),
      makeTool({ name: "read_file", categories: ["fs"] }),
      makeTool({ name: "grep", categories: ["fs"] }),
    ];
    render(<OverviewPage />);
    await waitFor(() => expect(screen.getByText("shell")).toBeInTheDocument());
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("grep")).toBeInTheDocument();
    expect(screen.getByText("system")).toBeInTheDocument();
    expect(screen.getByText("fs")).toBeInTheDocument();
  });

  it("does not crash on tools without categories", async () => {
    // El daemon puede mandar un item sin `categories`; no debe romper.
    mockApi.plugins = [
      makeTool({ name: "sin_cat", categories: undefined as unknown as string[] }),
    ];
    render(<OverviewPage />);
    await waitFor(() => expect(screen.getByText("sin_cat")).toBeInTheDocument());
  });

  it("flags a closed websocket as needing attention", () => {
    mockStore.ws = "closed";
    render(<OverviewPage />);
    expect(screen.getByText(/sin conexión con el daemon/i)).toBeInTheDocument();
  });

  it("shows a single error when the daemon is unreachable", async () => {
    mockApi.rejectAll = true;
    render(<OverviewPage />);
    await waitFor(() =>
      expect(screen.getByText(/no se pudo contactar el daemon/i)).toBeInTheDocument(),
    );
  });

  it("renders the empty state when there are no tools", async () => {
    render(<OverviewPage />);
    await waitFor(() =>
      expect(screen.getByText(/no hay tools registradas/i)).toBeInTheDocument(),
    );
  });
});