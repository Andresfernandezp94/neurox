// Tests de LocalServiceCard — la card que crea/configura y arranca/parra el
// servicio LLM local desde la tab Local.
//
// El backend ya soporta todo (POST/PUT /v1/llm/providers con los campos
// `local_*`, y /start + /stop vía el orquestador). Lo que se verifica acá es
// el cableado del front y, sobre todo, los caminos donde el operador podría
// perderse: puerto inválido, modelo inexistente, y la creación que declara
// un provider local que en realidad es remoto.

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/react";

vi.mock("../api/llm", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/llm")>();
  return {
    ...actual,
    getProviders: vi.fn(),
    addProvider: vi.fn(),
    updateProvider: vi.fn(),
    startProvider: vi.fn(),
    stopProvider: vi.fn(),
    deleteProvider: vi.fn(),
  };
});

import * as llmApi from "../api/llm";
import { LocalServiceCard, LOCAL_SERVICE_ID } from "./LocalServiceCard";
import type { LlmProviderStatus } from "../api/llm";

const chatModel = {
  filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
  path: "/models/chat/qwen2.5-1.5b-instruct-q4_k_m.gguf",
  size_bytes: 1_066_000_000,
  category: "chat",
};

function localProvider(over: Partial<LlmProviderStatus> = {}): LlmProviderStatus {
  return {
    id: LOCAL_SERVICE_ID,
    kind: "openai_compat",
    model: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    base_url: "http://127.0.0.1:8080/v1",
    configured: true,
    active: true,
    service_state: "stopped",
    local_command: "llama-server",
    local_args: ["--model", "{{model_path}}", "--port", "{{port}}"],
    local_model_path: chatModel.path,
    local_port: 8080,
    ...over,
  };
}

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.mocked(llmApi.addProvider).mockResolvedValue(localProvider() as never);
  vi.mocked(llmApi.updateProvider).mockResolvedValue(localProvider() as never);
  vi.mocked(llmApi.startProvider).mockResolvedValue({
    provider_id: LOCAL_SERVICE_ID,
    state: "running",
  } as never);
  vi.mocked(llmApi.stopProvider).mockResolvedValue({
    provider_id: LOCAL_SERVICE_ID,
    state: "stopped",
  } as never);
  vi.mocked(llmApi.deleteProvider).mockResolvedValue(undefined as never);
});

afterEach(() => cleanup());

describe("LocalServiceCard", () => {
  it("offers the creation form when no local service exists", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);

    await waitFor(() => {
      expect(screen.getByTestId("local-svc-create")).toBeInTheDocument();
    });
    // El mensaje viejo decía "configure one under the Providers tab": la
    // configuración vive acá, no en otra tab.
    expect(screen.queryByText(/Providers tab/i)).toBeNull();
    expect(screen.getByTestId("local-svc-model")).toBeInTheDocument();
    expect(screen.getByTestId("local-svc-port")).toBeInTheDocument();
    expect(screen.getByTestId("local-svc-command")).toBeInTheDocument();
  });

  it("creates the provider with llama-server defaults and the daemon placeholders", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);

    await waitFor(() => expect(screen.getByTestId("local-svc-create")).toBeInTheDocument());
    fireEvent.change(screen.getByTestId("local-svc-model"), {
      target: { value: chatModel.path },
    });
    fireEvent.click(screen.getByTestId("local-svc-create"));

    await waitFor(() => expect(llmApi.addProvider).toHaveBeenCalled());
    const payload = vi.mocked(llmApi.addProvider).mock.calls[0]?.[0];
    expect(payload).toBeDefined();
    if (!payload) throw new Error("addProvider was not called with a payload");
    expect(payload.id).toBe(LOCAL_SERVICE_ID);
    expect(payload.local_command).toBe("llama-server");
    expect(payload.local_port).toBe(8080);
    expect(payload.local_model_path).toBe(chatModel.path);
    // Los placeholders los expande el daemon: el front manda las plantillas,
    // no el path ya resuelto.
    const args = payload.local_args ?? [];
    expect(args).toContain("{{model_path}}");
    expect(args).toContain("{{port}}");
    // llama-server expone una API OpenAI-compatible: el resto de neurox lo
    // trata como openai_compat.
    expect(payload.kind).toBe("openai_compat");
    expect(payload.base_url).toBe("http://127.0.0.1:8080/v1");
  });

  it("binds to localhost by default, not 0.0.0.0", async () => {
    // Un modelo servido sin auth en la red local no puede ser el default.
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-create")).toBeInTheDocument());
    fireEvent.change(screen.getByTestId("local-svc-model"), {
      target: { value: chatModel.path },
    });
    fireEvent.click(screen.getByTestId("local-svc-create"));

    await waitFor(() => expect(llmApi.addProvider).toHaveBeenCalled());
    const args = vi.mocked(llmApi.addProvider).mock.calls[0]?.[0]?.local_args ?? [];
    expect(args).toContain("--host");
    const hostIdx = args.indexOf("--host");
    expect(args[hostIdx + 1]).toBe("127.0.0.1");
  });

  it("refuses an empty port instead of sending it", async () => {
    // `type="number"` descarta letras: escribir "abc" deja el campo vacío.
    // El caso real es que el campo quede en "" y el puerto sin definir, y
    // el botón tiene que seguir bloqueado.
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-create")).toBeInTheDocument());

    fireEvent.change(screen.getByTestId("local-svc-model"), {
      target: { value: chatModel.path },
    });
    fireEvent.change(screen.getByTestId("local-svc-port"), { target: { value: "" } });

    expect(screen.getByTestId("local-svc-create")).toBeDisabled();
    fireEvent.click(screen.getByTestId("local-svc-create"));
    expect(llmApi.addProvider).not.toHaveBeenCalled();
  });

  it("explains an out-of-range port to the operator", async () => {
    // `type="number"` acepta negativos y decimales; eso sí llega al estado y
    // tiene que producir un mensaje, no solo un botón deshabilitado.
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-create")).toBeInTheDocument());

    fireEvent.change(screen.getByTestId("local-svc-model"), {
      target: { value: chatModel.path },
    });
    fireEvent.change(screen.getByTestId("local-svc-port"), { target: { value: "-1" } });

    expect(screen.getByRole("alert")).toHaveTextContent(/entre 1 y 65535/i);
    expect(screen.getByTestId("local-svc-create")).toBeDisabled();
  });

  it("refuses an out-of-range port", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-create")).toBeInTheDocument());
    fireEvent.change(screen.getByTestId("local-svc-model"), {
      target: { value: chatModel.path },
    });
    fireEvent.change(screen.getByTestId("local-svc-port"), { target: { value: "70000" } });
    expect(screen.getByTestId("local-svc-create")).toBeDisabled();
  });

  it("says when there are no chat models instead of an empty dropdown", async () => {
    // Un embedding no se sirve con llama-server; ofrecerlo sería prometer
    // algo que el servicio no puede hacer.
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [],
      default_provider: "",
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[]} />);
    await waitFor(() => {
      expect(screen.getByTestId("local-svc-no-chat-models")).toBeInTheDocument();
    });
    expect(screen.getByTestId("local-svc-create")).toBeDisabled();
  });

  it("shows start/stop and the live state when configured", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [localProvider({ service_state: "running" })],
      default_provider: LOCAL_SERVICE_ID,
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);

    await waitFor(() => {
      expect(screen.getByTestId("local-svc-toggle")).toBeInTheDocument();
    });
    expect(screen.getByTestId("local-service-state")).toHaveTextContent("running");
    expect(screen.getByTestId("local-svc-save")).toBeInTheDocument();
  });

  it("stops a running service and starts a stopped one", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [localProvider({ service_state: "running" })],
      default_provider: LOCAL_SERVICE_ID,
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-toggle")).toBeInTheDocument());

    fireEvent.click(screen.getByTestId("local-svc-toggle"));
    await waitFor(() => {
      expect(llmApi.stopProvider).toHaveBeenCalledWith(LOCAL_SERVICE_ID);
    });

    cleanup();
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [localProvider({ service_state: "stopped" })],
      default_provider: LOCAL_SERVICE_ID,
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-toggle")).toBeInTheDocument());
    fireEvent.click(screen.getByTestId("local-svc-toggle"));
    await waitFor(() => {
      expect(llmApi.startProvider).toHaveBeenCalledWith(LOCAL_SERVICE_ID);
    });
  });

  it("warns when the service failed to start", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [localProvider({ service_state: "failed" })],
      default_provider: LOCAL_SERVICE_ID,
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/falló al arrancar/i);
    });
  });

  it("keeps a configured model visible in the select even if it is gone from disk", async () => {
    // Si el GGUF se borró, el select caería a la primera opción y el
    // operador creería que cambió de modelo al guardar.
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [localProvider({ local_model_path: "/models/chat/borrado.gguf" })],
      default_provider: LOCAL_SERVICE_ID,
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-model")).toBeInTheDocument());
    const select = screen.getByTestId("local-svc-model") as HTMLSelectElement;
    expect(select.value).toBe("/models/chat/borrado.gguf");
    expect(screen.getByText(/no encontrado/i)).toBeInTheDocument();
  });

  it("saves a changed port to the running provider", async () => {
    vi.mocked(llmApi.getProviders).mockResolvedValue({
      providers: [localProvider()],
      default_provider: LOCAL_SERVICE_ID,
      default_model: "",
    } as never);
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => expect(screen.getByTestId("local-svc-save")).toBeInTheDocument());

    fireEvent.change(screen.getByTestId("local-svc-port"), { target: { value: "9090" } });
    fireEvent.click(screen.getByTestId("local-svc-save"));

    await waitFor(() => {
      expect(llmApi.updateProvider).toHaveBeenCalledWith(
        LOCAL_SERVICE_ID,
        expect.objectContaining({ local_port: 9090 }),
      );
    });
  });

  it("surfaces an API error", async () => {
    vi.mocked(llmApi.getProviders).mockRejectedValue(new Error("boom"));
    render(<LocalServiceCard chatModels={[chatModel]} />);
    await waitFor(() => {
      expect(screen.getByText("boom")).toBeInTheDocument();
    });
  });
});