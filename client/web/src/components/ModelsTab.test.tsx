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

const localModel = {
  filename: "qwen2.5-1.5b-instruct-Q4_K_M.gguf",
  path: "/models/qwen2.5-1.5b-instruct-Q4_K_M.gguf",
  size_bytes: 1_100_000_000,
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
    expect(await screen.findByText("ready")).toBeInTheDocument();
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
    expect(await screen.findByText("stopped")).toBeInTheDocument();
    expect(screen.getByTestId("local-svc-toggle").title).toContain("Start");
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
    expect(screen.getByText(/not orchestrated by the daemon yet/i)).toBeInTheDocument();
  });
});