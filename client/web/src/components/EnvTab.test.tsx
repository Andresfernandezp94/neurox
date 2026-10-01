// Tests de EnvTab contra el catálogo del daemon.
//
// Lo que importa acá es que la UI respete las reglas del catálogo:
// - una variable de solo lectura no ofrece input de escritura
// - el valor nunca se renderiza (el daemon no lo manda, y el input es
//   password igual)
// - las categorías se agrupan en el orden que declara el daemon
// - un valor vacío no dispara un PUT

import { describe, expect, it, afterEach, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/react";

vi.mock("../api/env", async () => {
  const actual = await vi.importActual<typeof import("../api/env")>(
    "../api/env",
  );
  return {
    ...actual,
    getEnv: vi.fn(),
    putEnvVar: vi.fn(),
    deleteEnvVar: vi.fn(),
  };
});

import * as envApi from "../api/env";
import { EnvTab } from "./EnvTab";
import type { EnvResponse, EnvVar } from "../types";

function var_(over: Partial<EnvVar> & { key: string }): EnvVar {
  return {
    set: false,
    readOnly: false,
    sensitive: false,
    category: "runtime",
    categoryLabel: "Runtime",
    description: "desc",
    defaultValue: null,
    inFile: false,
    inEnv: false,
    ...over,
  };
}

const response: EnvResponse = {
  path: "/home/u/.config/neurox/env",
  categories: [
    { id: "runtime", label: "Runtime", description: "Config del daemon." },
    { id: "default-llm", label: "Default de LLM", description: "Override." },
    { id: "infrastructure", label: "Infraestructura", description: "Del host." },
  ],
  vars: [
    var_({
      key: "NEUROX_TODO_DIR",
      description: "Directorio del store de tareas.",
      defaultValue: null,
    }),
    var_({
      key: "NEUROX_MAX_TOOL_ITERATIONS",
      description: "Tope de iteraciones.",
      defaultValue: "10",
      set: true,
      inFile: true,
      inEnv: true,
    }),
    var_({
      key: "NEUROX_SESSION_TIMEOUT",
      description: "Timeout de sesion.",
      defaultValue: "180",
    }),
    var_({
      key: "MINIMAX_API_KEY",
      category: "default-llm",
      categoryLabel: "Default de LLM",
      sensitive: true,
      readOnly: true,
      set: true,
      inFile: true,
      inEnv: true,
      description: "API key de MiniMax.",
    }),
    var_({
      key: "PATH",
      category: "infrastructure",
      categoryLabel: "Infraestructura",
      readOnly: true,
      set: true,
      inEnv: true,
      description: "Ruta de ejecutables.",
    }),
  ],
  unknownVars: [
    var_({
      key: "MI_VAR",
      category: "custom",
      categoryLabel: "Personalizadas",
      set: true,
      inFile: true,
      description: "No pertenece al catalogo.",
    }),
  ],
};

beforeEach(() => {
  vi.mocked(envApi.getEnv).mockResolvedValue(response);
  vi.mocked(envApi.putEnvVar).mockResolvedValue(undefined);
  vi.mocked(envApi.deleteEnvVar).mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("EnvTab", () => {
  it("lists every catalog var, even ones never written to the file", async () => {
    render(<EnvTab />);
    // NEUROX_TODO_DIR has `set: false`: before the catalog existed, a var
    // that was never written did not appear at all, so there was nothing
    // to add from the UI.
    expect(await screen.findByText("NEUROX_TODO_DIR")).toBeInTheDocument();
    expect(screen.getByText("NEUROX_MAX_TOOL_ITERATIONS")).toBeInTheDocument();
  });

  it("renders categories in the order the daemon declares", async () => {
    render(<EnvTab />);
    const headings = await screen.findAllByRole("heading", { level: 4 });
    const labels = headings.map((h) => h.textContent?.trim());
    expect(labels[0]).toContain("Runtime");
    // "Default de LLM" declares before "Infraestructura".
    expect(labels.indexOf("Default de LLM")).toBeLessThan(
      labels.indexOf("Infraestructura"),
    );
  });

  it("does not offer a write input for a read-only var", async () => {
    render(<EnvTab />);
    // Provider keys live in the Providers tab; two write paths would
    // overwrite each other without warning.
    expect(await screen.findByText("MINIMAX_API_KEY")).toBeInTheDocument();
    expect(screen.queryByTestId("env-input-MINIMAX_API_KEY")).toBeNull();
    expect(screen.queryByTestId("env-save-MINIMAX_API_KEY")).toBeNull();
    expect(screen.getByText("Se edita en la tab Providers.")).toBeInTheDocument();

    // Infrastructure says why it is not editable.
    expect(screen.queryByTestId("env-input-PATH")).toBeNull();
    expect(
      screen.getByText("La define el host. Neurox no la escribe."),
    ).toBeInTheDocument();
  });

  it("never renders a value, only set/unset state", async () => {
    render(<EnvTab />);
    expect(await screen.findByText("NEUROX_MAX_TOOL_ITERATIONS")).toBeInTheDocument();
    // The daemon never sends values, so there is nothing to leak; what
    // must not happen is the UI implying it knows one.
    expect(screen.getAllByText("set").length).toBeGreaterThan(0);
    expect(screen.getAllByText("unset").length).toBeGreaterThan(0);
  });

  it("uses a password input for secrets", async () => {
    render(<EnvTab />);
    const input = await screen.findByTestId("env-input-NEUROX_TODO_DIR");
    expect(input).toHaveAttribute("type", "password");
  });

  it("shows the default only for a var that is unset", async () => {
    render(<EnvTab />);
    // NEUROX_SESSION_TIMEOUT is unset → the default is worth acting on.
    expect(await screen.findByText("180")).toBeInTheDocument();
    // NEUROX_MAX_TOOL_ITERATIONS is set: repeating the default would be
    // noise, the effective value is already the operator's.
    expect(screen.queryByText("10")).toBeNull();
  });

  it("saves a draft and reloads", async () => {
    render(<EnvTab />);
    const input = await screen.findByTestId("env-input-NEUROX_TODO_DIR");
    fireEvent.change(input, { target: { value: "/tmp/todos" } });
    fireEvent.click(screen.getByTestId("env-save-NEUROX_TODO_DIR"));

    await waitFor(() => {
      expect(envApi.putEnvVar).toHaveBeenCalledWith(
        "NEUROX_TODO_DIR",
        "/tmp/todos",
      );
    });
    // Reload after write so the card reflects the new state.
    await waitFor(() => {
      expect(vi.mocked(envApi.getEnv).mock.calls.length).toBeGreaterThan(1);
    });
  });

  it("does not PUT an empty draft", async () => {
    render(<EnvTab />);
    // An empty string is not a value; writing it would clear a set var by
    // accident when the operator just clicked through the input.
    const input = await screen.findByTestId("env-input-NEUROX_TODO_DIR");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.click(screen.getByTestId("env-save-NEUROX_TODO_DIR"));
    expect(envApi.putEnvVar).not.toHaveBeenCalled();
  });

  it("keeps save disabled until there is something to save", async () => {
    render(<EnvTab />);
    const save = await screen.findByTestId("env-save-NEUROX_TODO_DIR");
    expect(save).toBeDisabled();
    fireEvent.change(screen.getByTestId("env-input-NEUROX_TODO_DIR"), {
      target: { value: "x" },
    });
    expect(save).not.toBeDisabled();
  });

  it("offers clear only for a var that is set", async () => {
    render(<EnvTab />);
    expect(
      await screen.findByTestId("env-clear-NEUROX_MAX_TOOL_ITERATIONS"),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("env-clear-NEUROX_TODO_DIR")).toBeNull();
  });

  it("lists hand-written vars as custom and still editable", async () => {
    render(<EnvTab />);
    expect(await screen.findByText("MI_VAR")).toBeInTheDocument();
    // Not in the catalog, but the operator wrote it by hand: the UI must
    // not hide it, nor make it read-only.
    expect(screen.getByTestId("env-input-MI_VAR")).toBeInTheDocument();
  });

  it("shows the env file path", async () => {
    render(<EnvTab />);
    expect(await screen.findByText("/home/u/.config/neurox/env")).toBeInTheDocument();
  });

  it("surfaces an API error", async () => {
    vi.mocked(envApi.getEnv).mockRejectedValue(new Error("boom"));
    render(<EnvTab />);
    expect(await screen.findByText("boom")).toBeInTheDocument();
  });

  it("surfaces a 403 when writing a read-only var", async () => {
    // The daemon rejects with 403; the UI has to show it, not swallow it.
    vi.mocked(envApi.putEnvVar).mockRejectedValue(
      new Error("'PATH' es de solo lectura"),
    );
    render(<EnvTab />);
    const input = await screen.findByTestId("env-input-NEUROX_TODO_DIR");
    fireEvent.change(input, { target: { value: "/x" } });
    fireEvent.click(screen.getByTestId("env-save-NEUROX_TODO_DIR"));
    // El mensaje va en el banner de error, no en las cards.
    expect(
      await screen.findByText("'PATH' es de solo lectura"),
    ).toBeInTheDocument();
  });
});