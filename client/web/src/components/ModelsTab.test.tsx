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

// El servicio local que administra LocalServiceCard. El id tiene que
// coincidir con `LOCAL_SERVICE_ID`: la card busca el provider por id, y con
// otro id no encuentra ninguno y muestra el formulario de creacion en vez
// del switch.
const ollamaStopped: LlmProviderStatus = {
  id: "local-llama-server",
  kind: "openai_compat",
  model: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
  base_url: "http://127.0.0.1:8080/v1",
  api_key_env: null,
  configured: true,
  active: false,
  service_state: "stopped",
  local_command: "llama-server",
  local_port: 8080,
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
  it("renderiza los modelos locales como filas seleccionables", async () => {
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
    const names = await screen.findAllByText("qwen2.5-1.5b-instruct-Q4_K_M.gguf");
    // Aparece en la card del servicio local (como modelo configurado) y en
    // la lista de GGUF. Lo que se verifica es que esté en la lista.
    expect(names.length).toBeGreaterThan(0);
    expect(screen.getByText("1.02 GB")).toBeInTheDocument();
    // La fila es un botón: el área clickeable es toda la fila, no un icono.
    expect(
      screen.getByTestId("model-row-qwen2.5-1.5b-instruct-Q4_K_M.gguf"),
    ).toHaveAttribute("type", "button");
    // El set-as-active vive en el panel de config, no en cada fila.
    expect(screen.queryByTestId("set-active-button")).toBeNull();
  });

  it("Start arranca el servicio local (ollama serve) y cambia a Stop", async () => {
    let state: LlmProviderStatus["service_state"] = "stopped";
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "MODELS_DIR",
      models: [],
    } as never);
    vi.mocked(llmApi.getProviders).mockImplementation(async () => ({
      providers: [{ ...ollamaStopped, id: "local-llama-server", service_state: state }],
      default_model: "",
    } as never));
    vi.mocked(llmApi.startProvider).mockImplementation(async () => {
      state = "ready";
      return { id: "local-llama-server", service_state: "ready" } as never;
    });

    render(<ModelsTab />);
    const startBtn = await screen.findByTestId("local-svc-toggle");
    expect(startBtn.title).toContain("Start");

    fireEvent.click(startBtn);
    await waitFor(() => expect(llmApi.startProvider).toHaveBeenCalledWith("local-llama-server"));
    // El estado real del servicio, no un badge genérico "ACTIVE": antes la
    // card decía ACTIVE sin decir si el proceso estaba vivo.
    await waitFor(() => {
      expect(screen.getByTestId("local-service-state")).toHaveTextContent("ready");
    });
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
      providers: [{ ...ollamaStopped, id: "local-llama-server", service_state: state }],
      default_model: "",
    } as never));
    vi.mocked(llmApi.stopProvider).mockImplementation(async () => {
      state = "stopped";
      return { id: "local-llama-server", service_state: "stopped" } as never;
    });

    render(<ModelsTab />);
    const stopBtn = await screen.findByTestId("local-svc-toggle");
    expect(stopBtn.title).toContain("Stop");

    fireEvent.click(stopBtn);
    await waitFor(() => expect(llmApi.stopProvider).toHaveBeenCalledWith("local-llama-server"));
    expect(await screen.findByTestId("local-svc-toggle")).toBeInTheDocument();
    expect(screen.getByTestId("local-svc-toggle").title).toContain("Start");
    expect(screen.queryByText("ACTIVE")).not.toBeInTheDocument();
  });

  it("sin provider local ofrece el formulario de creacion, no un hint a otra tab", async () => {
    // Antes decia "configure one under the Providers tab". La configuracion
    // del servicio local vive en esta misma tab.
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
    expect(await screen.findByTestId("local-svc-create")).toBeInTheDocument();
    expect(screen.queryByText(/no local service configured/i)).toBeNull();
    expect(screen.queryByText(/Providers tab/i)).toBeNull();
  });

  it("no ofrece switch para un Ollama loopback sin local_command", async () => {
    // Un provider remoto que escucha en loopback no es el servicio local que
    // neurox orquesta: no tiene `local_command`, asi que no hay nada que
    // arrancar/parar. Mostrarle un switch seria mentir sobre quien lo
    // controla.
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
    await waitFor(() => {
      expect(screen.getByTestId("local-svc-create")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("local-svc-toggle")).toBeNull();
  });
});
// ─── Categorías por carpeta (EP-2026-10) ──────────────────────────────
//
// La carpeta del modelo bajo MODELS_DIR ES la categoría. El listado del
// daemon ya viene agrupado y ordenado, así que la UI agrupa sin reordenar.

describe("ModelsTab inline configuration panel", () => {
  const chatA = {
    filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    path: "/models/chat/qwen2.5-1.5b-instruct-q4_k_m.gguf",
    size_bytes: 1_066_000_000,
    category: "chat",
  };
  const chatB = {
    filename: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    path: "/models/chat/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    size_bytes: 2_382_000_000,
    category: "chat",
  };

  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    vi.mocked(llmApi.updateProvider).mockResolvedValue({} as never);
    vi.mocked(modelsApi.putModelConfig).mockResolvedValue({} as never);
  });

  function mockLocal() {
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "NEUROX_MODELS_DIR",
      models: [chatA, chatB],
    } as never);
  }

  it("keeps the panel visible with no selection instead of hiding it", async () => {
    // Si la caja desaparece, el operador no sabe que puede configurar nada.
    mockLocal();
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByTestId("configure-panel")).toBeInTheDocument();
    });
    expect(screen.getByText("No model selected")).toBeInTheDocument();
    expect(
      screen.getByText(/pick a model on the left/i),
    ).toBeInTheDocument();
  });

  it("renders rows with the project's own typography, not the browser default", async () => {
    // El proyecto no tiene reset global de `button`. Una fila <button> sin
    // `font: inherit` se renderiza con la tipografía por defecto del
    // navegador y con el fondo gris del user agent: se ve como una card sin
    // estilos. Esta clase es la que lo previene.
    mockLocal();
    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    const row = screen.getByTestId(
      "model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf",
    );
    expect(row.className).toContain("models-tab__model-row");
    // El contenedor es el que lleva el gap entre filas; sin el, los
    // <button> secuenciales se leen pegados.
    expect(row.parentElement).toHaveClass("models-tab__model-list");
  });

  it("keeps every row focusable for keyboard navigation", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({} as never);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    // Seleccionar con Enter tiene que funcionar igual que con click: es un
    // <button> nativo, no un div con onClick.
    const row = screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf");
    row.focus();
    fireEvent.keyDown(row, { key: "Enter" });
    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toBeInTheDocument();
    });
  });

  it("loads and shows the config of the model the operator clicked", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({
      temperature: 0.7,
      max_tokens: 4096,
    } as never);

    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
    );

    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toHaveValue(0.7);
    });
    expect(screen.getByTestId("cfg-max-tokens")).toHaveValue(4096);
    expect(modelsApi.getModelConfig).toHaveBeenCalledWith(chatA.filename);
  });

  it("switches config when another model is selected, without a modal", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockImplementation(async (f: string) =>
      ({ temperature: f === chatA.filename ? 0.3 : 0.9 }) as never
    );

    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toHaveValue(0.3);
    });

    fireEvent.click(
      screen.getByTestId("model-row-Qwen3-4B-Instruct-2507-Q4_K_M.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toHaveValue(0.9);
    });
    // Nunca hay overlay: comparar dos modelos no requiere cerrar nada.
    expect(document.querySelector(".modal-backdrop")).toBeNull();
  });

  it("marks the selected row so the operator knows which one is being edited", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({} as never);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });

    const rowA = screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf");
    const rowB = screen.getByTestId("model-row-Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    expect(rowA).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(rowA);
    await waitFor(() => {
      expect(rowA).toHaveAttribute("aria-pressed", "true");
    });
    expect(rowB).toHaveAttribute("aria-pressed", "false");
  });

  it("saves the config of the selected model", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({
      temperature: 0.7,
    } as never);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toHaveValue(0.7);
    });

    fireEvent.change(screen.getByTestId("cfg-temperature"), {
      target: { value: "0.2" },
    });
    fireEvent.click(screen.getByTestId("cfg-save"));

    await waitFor(() => {
      expect(modelsApi.putModelConfig).toHaveBeenCalledWith(
        chatA.filename,
        expect.objectContaining({ temperature: 0.2 }),
      );
    });
  });

  it("sets the selected model as active from the panel", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({} as never);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("set-active-button")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId("set-active-button"));

    await waitFor(() => {
      // El id lo define LocalServiceCard; antes vivia hardcodeado aca
      // como "local-llama" y ya no coincidia con el servicio real.
      expect(llmApi.updateProvider).toHaveBeenCalledWith("local-llama-server", {
        local_model_path: chatA.path,
      });
    });
  });

  it("does not leak a slow response into a newly selected model", async () => {
    mockLocal();
    vi.mocked(modelsApi.getModelConfig).mockImplementation(
      (f: string) =>
        new Promise((resolve) =>
          f === chatA.filename
            ? setTimeout(() => resolve({ temperature: 0.1 } as never), 50)
            : resolve({ temperature: 0.9 } as never),
        ),
    );
    render(<ModelsTab />);
    await waitFor(() => {
      expect(
        screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
      ).toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-1.5b-instruct-q4_k_m.gguf"),
    );
    fireEvent.click(
      screen.getByTestId("model-row-Qwen3-4B-Instruct-2507-Q4_K_M.gguf"),
    );
    // El fetch lento del primero no debe pisar el valor del segundo.
    await waitFor(
      () => {
        expect(screen.getByTestId("cfg-temperature")).toHaveValue(0.9);
      },
      { timeout: 500 },
    );
  });
});

describe("ModelsTab config persistence against the real API shape", () => {
  const m = {
    filename: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
    path: "/models/chat/qwen2.5-0.5b-instruct-q4_k_m.gguf",
    size_bytes: 468_000_000,
    category: "chat",
  };

  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "NEUROX_MODELS_DIR",
      models: [m],
    } as never);
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
  });

  it("reads the config from the row shape the daemon returns", async () => {
    // El daemon devuelve `{ config, filename, updated_at }`, no el
    // ModelConfig pelado. Sin desenvolver, todos los campos salían
    // undefined y el panel se veía vacío con config ya guardada.
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({
      temperature: 0.42,
      system_prompt: "soy un system prompt",
    } as never);

    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByTestId("model-row-qwen2.5-0.5b-instruct-q4_k_m.gguf"))
        .toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-0.5b-instruct-q4_k_m.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toHaveValue(0.42);
    });
    expect(screen.getByTestId("cfg-system")).toHaveValue(
      "soy un system prompt",
    );
  });

  it("sends the system prompt under the field name the daemon expects", async () => {
    // `system_prompt`, no `system`: el daemon descarta campos desconocidos
    // en silencio y el PUT respondía 200 guardando solo el resto.
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({} as never);
    vi.mocked(modelsApi.putModelConfig).mockResolvedValue({} as never);

    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByTestId("model-row-qwen2.5-0.5b-instruct-q4_k_m.gguf"))
        .toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-0.5b-instruct-q4_k_m.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("cfg-system")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByTestId("cfg-system"), {
      target: { value: "nuevo prompt" },
    });
    fireEvent.click(screen.getByTestId("cfg-save"));

    await waitFor(() => {
      expect(modelsApi.putModelConfig).toHaveBeenCalledWith(
        m.filename,
        expect.objectContaining({ system_prompt: "nuevo prompt" }),
      );
    });
    expect(modelsApi.putModelConfig).not.toHaveBeenCalledWith(
      m.filename,
      expect.objectContaining({ system: expect.anything() }),
    );
  });

  it("handles an empty config for a model that was never configured", async () => {
    // El daemon devuelve `{}` cuando no hay fila: es el estado inicial de
    // cualquier modelo recien descargado, no un error.
    vi.mocked(modelsApi.getModelConfig).mockResolvedValue({} as never);
    render(<ModelsTab />);
    await waitFor(() => {
      expect(screen.getByTestId("model-row-qwen2.5-0.5b-instruct-q4_k_m.gguf"))
        .toBeInTheDocument();
    });
    fireEvent.click(
      screen.getByTestId("model-row-qwen2.5-0.5b-instruct-q4_k_m.gguf"),
    );
    await waitFor(() => {
      expect(screen.getByTestId("cfg-temperature")).toHaveValue(null);
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("ModelsTab hugging face results", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.mocked(modelsApi.getLocalModels).mockResolvedValue({
      dir: "/models",
      env_var: "NEUROX_MODELS_DIR",
      models: [],
    } as never);
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
  });

  function hfModel(i: number) {
    return {
      id: `org/model-${i}`,
      display_name: `Model ${i}`,
      gated: false,
      downloads: 1000 + i,
    };
  }

  it("renders every result and scrolls inside a fixed-height wrapper", async () => {
    // Lo que se VE son 3 (HF_VISIBLE), pero los 30 se renderizan y el
    // wrapper scrollea. Si se hiciera `slice(0, 3)` la lista no tendria nada
    // que scrollear.
    const many = Array.from({ length: 30 }, (_, i) => hfModel(i));
    vi.mocked(modelsApi.searchHfModels).mockResolvedValue({
      models: many,
    } as never);

    render(<ModelsTab />);
    const search = await screen.findByPlaceholderText(/search hugging face/i);
    fireEvent.change(search, { target: { value: "qwen" } });

    await waitFor(() => {
      expect(screen.getByTestId("models-hf-list")).toBeInTheDocument();
    });
    // El wrapper con scroll existe y envuelve la lista.
    const wrapper = screen.getByTestId("models-hf-scroll");
    expect(wrapper).toContainElement(screen.getByTestId("models-hf-list"));
    // Los 30 results existen en el DOM.
    expect(screen.getByText("Model 0")).toBeInTheDocument();
    expect(screen.getByText("Model 29")).toBeInTheDocument();
    expect(screen.getAllByText(/^Model /)).toHaveLength(30);
  });

  it("still reports the real total when results fit", async () => {
    vi.mocked(modelsApi.searchHfModels).mockResolvedValue({
      models: [hfModel(0), hfModel(1)],
    } as never);

    render(<ModelsTab />);
    const search = await screen.findByPlaceholderText(/search hugging face/i);
    fireEvent.change(search, { target: { value: "qwen" } });

    await waitFor(() => {
      expect(screen.getByTestId("models-hf-list")).toBeInTheDocument();
    });
    expect(screen.getByTestId("hf-results-badge")).toHaveTextContent("2 results");
  });
});

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
      // Puede aparecer también como modelo configurado del servicio local.
      expect(screen.getAllByText("qwen.gguf").length).toBeGreaterThan(0);
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
