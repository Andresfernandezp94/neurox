// Tests de ModelsTab — listado local GGUF + control Start/Stop del
// servicio local (ollama serve) + HF download. EP-0020-02 / EP-0018-04.

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/react";

vi.mock("../api/models", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/models")>();
  return {
    ...actual,
    getLocalModels: vi.fn(),
    searchHfModels: vi.fn(),
    downloadModel: vi.fn(),
    getModelConfig: vi.fn(),
    putModelConfig: vi.fn(),
  };
});

vi.mock("../api/llm", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/llm")>();
  return {
    ...actual,
    getProviders: vi.fn(),
    updateProvider: vi.fn(),
    startProvider: vi.fn(),
    stopProvider: vi.fn(),
  };
});

import * as modelsApi from "../api/models";
import * as llmApi from "../api/llm";
import { ModelsTab } from "./ModelsTab";
import type { LlmProviderStatus } from "../api/llm";

// La categoria viene del daemon: es la carpeta del modelo bajo
// MODELS_DIR. Un GGUF en la raiz llega como "unclassified".
const localModel = {
  filename: "qwen2.5-1.5b-instruct-Q4_K_M.gguf",
  path: "/models/chat/qwen2.5-1.5b-instruct-Q4_K_M.gguf",
  size_bytes: 1_100_000_000,
  category: "chat",
};

const ollamaStopped: LlmProviderStatus = {
  id: "local-ollama",
  kind: "ollama",
  model: "qwen2.5-1.5b-instruct",
  base_url: "http://127.0.0.1:11434/v1",
  api_key_env: null,
  configured: true,
  active: false,
  service_state: "stopped",
  local_command: "ollama serve",
};

// Ollama externo (servicio del sistema): loopback, sin local_command ni
// service_state → la card igual debe renderizarse (paso 1).
const ollamaExternal: LlmProviderStatus = {
  ...ollamaStopped,
  id: "ollama",
  local_command: null,
  service_state: null,
};

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.mocked(modelsApi.searchHfModels).mockResolvedValue({
    models: [],
  } as never);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ModelsTab", () => {
  it("renderiza los modelos locales como cards estilo providers", async () => {
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "MODELS_DIR",
      models: [localModel],
    } as never);
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [ollamaStopped],
      default_model: "",
    } as never);
    render(<ModelsTab />);
    expect(await screen.findByText("qwen2.5-1.5b-instruct-Q4_K_M.gguf")).toBeInTheDocument();
    expect(screen.getByText("1.02 GB")).toBeInTheDocument();
    expect(screen.getByTestId("set-active-button")).toBeInTheDocument();
  });

  it("Start arranca el servicio local (ollama serve) y cambia a Stop", async () => {
    let state: LlmProviderStatus["service_state"] = "stopped";
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "MODELS_DIR",
      models: [],
    } as never);
    vi.mocked(llmApi.getProviders).mockImplementation(async () => ({
      providers: [{ ...ollamaStopped, service_state: state }],
      default_model: "",
    } as never));
    vi.mocked(llmApi.startProvider).mockImplementation(async () => {
      state = "ready";
      return { id: "local-ollama", service_state: "ready" } as never;
    });

    render(<ModelsTab />);
    const startBtn = await screen.findByTestId("local-svc-toggle");
    expect(startBtn.title).toContain("Start");

    fireEvent.click(startBtn);
    await waitFor(() => expect(llmApi.startProvider).toHaveBeenCalledWith("local-ollama"));
    expect(await screen.findByText("ACTIVE")).toBeInTheDocument();
    expect(screen.getByTestId("local-svc-toggle").title).toContain("Stop");
  });

  it("Stop apaga el servicio local", async () => {
    let state: LlmProviderStatus["service_state"] = "ready";
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "MODELS_DIR",
      models: [],
    } as never);
    vi.mocked(llmApi.getProviders).mockImplementation(async () => ({
      providers: [{ ...ollamaStopped, service_state: state }],
      default_model: "",
    } as never));
    vi.mocked(llmApi.stopProvider).mockImplementation(async () => {
      state = "stopped";
      return { id: "local-ollama", service_state: "stopped" } as never;
    });

    render(<ModelsTab />);
    const stopBtn = await screen.findByTestId("local-svc-toggle");
    expect(stopBtn.title).toContain("Stop");

    fireEvent.click(stopBtn);
    await waitFor(() => expect(llmApi.stopProvider).toHaveBeenCalledWith("local-ollama"));
    expect(await screen.findByTestId("local-svc-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("local-svc-toggle").title).toContain("Start");
    expect(screen.queryByText("ACTIVE")).not.toBeInTheDocument();
  });

  it("sin provider local muestra el hint de que no hay servicio configurado", async () => {
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "MODELS_DIR",
      models: [],
    } as never);
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_model: "",
    } as never);
    render(<ModelsTab />);
    expect(
      await screen.findByText(/no local service configured/i),
    ).toBeInTheDocument();
  });

  it("renderiza la card para un Ollama loopback sin local_command (no orquestado)", async () => {
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "MODELS_DIR",
      models: [],
    } as never);
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [ollamaExternal],
      default_model: "",
    } as never);
    render(<ModelsTab />);
    expect(await screen.findByText("ollama")).toBeInTheDocument();
    expect(await screen.findByTestId("local-svc-toggle")).toBeInTheDocument();
  });
});
// ─── Categorías por carpeta (EP-2026-10) ──────────────────────────────
//
// La carpeta del modelo bajo MODELS_DIR ES la categoría. El listado del
// daemon ya viene agrupado y ordenado, así que la UI agrupa sin reordenar.

describe("ModelsTab categories", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.mocked(modelsApi.searchHfModels).mockResolvedValue({ models: [] } as never);
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
  });

  function mockLocal(dir: string | null, models: unknown[]) {
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir,
      env_var: "NEUROX_MODELS_DIR",
      models,
    } as never);
  }

  const m = (filename: string, category: string, size = 100) => ({
    filename,
    path: `/models/${category}/${filename}`,
    size_bytes: size,
    category,
  });

  it("groups models by their folder category", async () => {
    mockLocal("/models", [
      m("qwen.gguf", "chat", 2_382_000_000),
      m("bge-m3.gguf", "embedding", 581_000_000),
      m("bge-large.gguf", "embedding", 639_000_000),
    ]);
    render(<ModelsTab />);

    await waitFor(() => {
      expect(screen.getByText("qwen.gguf")).toBeInTheDocument();
    });
    expect(screen.getByText("bge-m3.gguf")).toBeInTheDocument();
    expect(screen.getByText("bge-large.gguf")).toBeInTheDocument();

    // Un header por categoría, con su conteo.
    expect(screen.getByText("Chat")).toBeInTheDocument();
    expect(screen.getByText("Embedding")).toBeInTheDocument();
    // 3 modelos en 2 categorías.
    expect(screen.getByText("3 in 2 categories")).toBeInTheDocument();
  });

  it("labels a root-level gguf as unclassified, not as a real category", async () => {
    // El operador tiene que poder distinguir "no lo organicé" de una
    // categoría de verdad; mostrarlo como categoría lo disimula.
    mockLocal("/models", [m("suelto.gguf", "unclassified")]);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByText("Sin categoria")).toBeInTheDocument();
    });
  });

  it("shows a category the UI has no label for, verbatim", async () => {
    // El operador puede crear las carpetas que quiera. La UI respeta su
    // nomenclatura en vez de deformarla a un set cerrado.
    mockLocal("/models", [m("x.gguf", "vision-special")]);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByText("vision-special")).toBeInTheDocument();
    });
  });

  it("says the directory is missing instead of claiming it is empty", async () => {
    // `dir: null` = el directorio no existe. Decir "vacío, descargá uno"
    // manda al operador a una descarga de 4GB cuando el problema es un path.
    mockLocal(null, []);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByText("Models directory not found")).toBeInTheDocument();
    });
    expect(screen.getByText(/does not exist/i)).toBeInTheDocument();
    expect(screen.queryByText("No local models")).toBeNull();
  });

  it("says the directory is empty only when it exists", async () => {
    mockLocal("/models", []);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByText("No local models")).toBeInTheDocument();
    });
    // El hint del empty state dice qué hacer, en vez de repetir el path.
    expect(screen.getByText(/has no \.gguf files/i)).toBeInTheDocument();
  });

  it("shows the resolved directory path", async () => {
    mockLocal("/home/u/tools/models", [m("a.gguf", "chat")]);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByText("/home/u/tools/models")).toBeInTheDocument();
    });
  });
});
