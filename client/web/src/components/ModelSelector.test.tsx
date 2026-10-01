// EP-0024: ModelSelector is a custom dropdown (button + listbox), not
// a <select> with <optgroup>. These tests were rewritten to drive
// the dropdown UI (click trigger → assert listbox items).

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ModelSelector } from "./ModelSelector";

// El selector consume el CATÁLOGO AGREGADO (`GET /v1/llm/models`), no un
// fetch por provider. Antes mockeábamos getProviders+listModels porque el
// selector hacía N requests a APIs de terceros en paralelo; con 8 providers
// eso es 8 requests por cada apertura del dropdown.
vi.mock("../api/llm", () => ({
  getModelCatalog: vi.fn(),
  setSessionModel: vi.fn(),
  getLlmPrefs: vi.fn(),
  setLlmPrefs: vi.fn(),
}));

import {
  getModelCatalog,
  setSessionModel,
  getLlmPrefs,
  setLlmPrefs,
} from "../api/llm";

const mockGetCatalog = getModelCatalog as ReturnType<typeof vi.fn>;
const mockSetSessionModel = setSessionModel as ReturnType<typeof vi.fn>;
const mockGetPrefs = getLlmPrefs as ReturnType<typeof vi.fn>;
const mockSetPrefs = setLlmPrefs as ReturnType<typeof vi.fn>;

const SID = "test-session-123";

// El catálogo agregado ya viene con provider_id y model_id resueltos, así
// que no hace falta reconstruirlo desde providers + per-provider fetches.
// `gpt-4o` aparece en dos providers a propósito: el selector tiene que
// desambiguar por provider_id, no solo por model_id.
const catalogResp = {
  models: [
    { provider_id: "minimax", model_id: "MiniMax-M3", kind: "minimax", base_url: "https://api.minimaxi.chat/v1", capability: "text-generation", supports_tools: true },
    { provider_id: "minimax", model_id: "MiniMax-M2.7", kind: "minimax", base_url: "https://api.minimaxi.chat/v1", capability: "text-generation", supports_tools: true },
    { provider_id: "openai", model_id: "gpt-4o", kind: "openai_compat", base_url: "https://api.openai.com/v1", capability: "text-generation", supports_tools: true },
    { provider_id: "openai", model_id: "gpt-4o-mini", kind: "openai_compat", base_url: "https://api.openai.com/v1", capability: "text-generation", supports_tools: true },
    { provider_id: "openrouter", model_id: "gpt-4o", kind: "openai_compat", base_url: "https://openrouter.ai/api/v1", capability: "text-generation", supports_tools: false },
  ],
  cached: true,
  fetched_at: "2026-10-01T00:00:00Z",
};

// Open the dropdown by clicking the trigger. The trigger is
// disabled while the catalog is loading, so we wait for it to
// become enabled first.
async function openDropdown() {
  await waitFor(() => {
    const trigger = screen.getByTestId("model-selector-trigger");
    expect(trigger).not.toBeDisabled();
  });
  fireEvent.click(screen.getByTestId("model-selector-trigger"));
}

describe("ModelSelector", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetCatalog.mockResolvedValue(catalogResp);
    // Por default: el usuario nunca eligió nada y el daemon devolvió el
    // primer provider con key. Los tests que cubren la rehidratación
    // sobreescriben esto.
    mockGetPrefs.mockResolvedValue({
      providerId: "minimax",
      model: "MiniMax-M3",
      source: "auto-configured",
      explicit: false,
    });
    mockSetPrefs.mockResolvedValue({
      providerId: "openai",
      model: "gpt-4o",
      source: "user",
      explicit: true,
    });
    mockSetSessionModel.mockResolvedValue({
      session_id: SID,
      provider_id: "openai",
      model: "gpt-4o",
    });
  });

  it("renders the catalog grouped by provider", async () => {
    render(<ModelSelector sessionId={SID} />);
    await openDropdown();
    // Three provider groups should be in the dropdown.
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-group-minimax")).toBeInTheDocument();
    });
    expect(screen.getByTestId("model-selector-group-openai")).toBeInTheDocument();
    expect(screen.getByTestId("model-selector-group-openrouter")).toBeInTheDocument();
  });

  // EP-2026-09-02: no default model. The selector shows the
  // "select model" placeholder until the user explicitly picks one
  // (was: auto-pick the daemon's default_provider/default_model).
  it("shows 'select model' when the user has no preference and none is resolved", async () => {
    // Sin preferencia persistida, el placeholder sigue siendo "select
    // model": es la señal de que hay que actuar.
    mockGetPrefs.mockResolvedValue({
      providerId: null,
      model: null,
      source: "none-configured",
      explicit: false,
    });
    render(<ModelSelector sessionId={SID} />);
    await waitFor(() => {
      const trigger = screen.getByTestId("model-selector-trigger");
      expect(trigger.textContent).toContain("select model");
    });
  });

  it("rehydrates the stored preference without a currentModel", async () => {
    // El bug que arregla /v1/llm/prefs: la selección vivía en useState y
    // se perdía al recargar. Ahora, sin currentModel, el trigger muestra
    // lo que el usuario tenía guardado.
    render(<ModelSelector sessionId={SID} />);
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-trigger").textContent).toContain(
        "MiniMax-M3",
      );
    });
  });

  it("marks a rehydrated default as not user-chosen", async () => {
    // Un default silencioso es indistinguible de una elección propia: el
    // operador no puede saber si su selección se respeta o no.
    render(<ModelSelector sessionId={SID} />);
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-pref-source")).toBeInTheDocument();
    });
  });

  it("does not mark an explicit preference as a default", async () => {
    mockGetPrefs.mockResolvedValue({
      providerId: "minimax",
      model: "MiniMax-M3",
      source: "user",
      explicit: true,
    });
    render(<ModelSelector sessionId={SID} />);
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-trigger").textContent).toContain(
        "MiniMax-M3",
      );
    });
    expect(screen.queryByTestId("model-selector-pref-source")).toBeNull();
  });

  it("persists the choice through /v1/llm/prefs, not only the session", async () => {
    // Si solo se guardara en la sesión, abrir otra pestaña pierde el
    // modelo elegido.
    render(<ModelSelector sessionId={SID} />);
    await openDropdown();
    // Los grupos arrancan plegados: hay que expandir el provider antes
    // de que el modelo sea clickeable.
    fireEvent.click(screen.getByTestId("model-selector-group-openai"));
    fireEvent.click(
      await screen.findByTestId("model-selector-item-openai::gpt-4o"),
    );
    await waitFor(() => {
      expect(mockSetPrefs).toHaveBeenCalledWith("openai", "gpt-4o");
    });
    expect(mockSetSessionModel).toHaveBeenCalledWith(SID, "openai", "gpt-4o");
  });

  it("preselects the currentModel when provided", async () => {
    render(
      <ModelSelector
        sessionId={SID}
        currentModel={{ provider_id: "openai", model: "gpt-4o" }}
      />,
    );
    // Trigger label uses the user-friendly name (`formatModelName`
    // replaces `-` with a space).
    await waitFor(() => {
      const trigger = screen.getByTestId("model-selector-trigger");
      expect(trigger.textContent).toContain("gpt-4o");
    });
  });

  it("disambiguates model_id collisions between providers", async () => {
    render(<ModelSelector sessionId={SID} />);
    await openDropdown();
    // Expand the openai and openrouter groups so the items render.
    fireEvent.click(screen.getByTestId("model-selector-group-openai"));
    fireEvent.click(screen.getByTestId("model-selector-group-openrouter"));
    // Both gpt-4o entries exist (one in openai, one in openrouter).
    expect(
      screen.getByTestId("model-selector-item-openai::gpt-4o"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("model-selector-item-openrouter::gpt-4o"),
    ).toBeInTheDocument();
    // The unique gpt-4o-mini is only in openai.
    expect(
      screen.getByTestId("model-selector-item-openai::gpt-4o-mini"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("model-selector-item-openrouter::gpt-4o-mini"),
    ).not.toBeInTheDocument();
  });

  it("calls setSessionModel on change and applies optimistic update", async () => {
    const onChange = vi.fn();
    render(<ModelSelector sessionId={SID} onChange={onChange} />);
    await openDropdown();
    fireEvent.click(screen.getByTestId("model-selector-group-openai"));
    fireEvent.click(
      screen.getByTestId("model-selector-item-openai::gpt-4o"),
    );
    await waitFor(() => {
      expect(mockSetSessionModel).toHaveBeenCalledWith(SID, "openai", "gpt-4o");
    });
    await waitFor(() => {
      expect(onChange).toHaveBeenCalledWith({
        provider_id: "openai",
        model: "gpt-4o",
      });
    });
    // Trigger label reflects the new selection.
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-trigger").textContent).toContain("gpt-4o");
    });
  });

  it("reverts to the previous selection when PUT fails", async () => {
    mockSetSessionModel.mockRejectedValueOnce(new Error("backend boom"));
    render(<ModelSelector sessionId={SID} currentModel={{ provider_id: "minimax", model: "MiniMax-M3" }} />);
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-trigger").textContent).toContain("MiniMax-M3");
    });
    await openDropdown();
    fireEvent.click(screen.getByTestId("model-selector-group-openai"));
    fireEvent.click(
      screen.getByTestId("model-selector-item-openai::gpt-4o"),
    );
    // Wait for the failed PUT to surface.
    await waitFor(() => {
      expect(screen.getByText("backend boom")).toBeInTheDocument();
    });
    // Selection reverted.
    expect(screen.getByTestId("model-selector-trigger").textContent).toContain("MiniMax-M3");
  });

  it("renders a loading hint before the catalog arrives", () => {
    mockGetCatalog.mockImplementation(
      () => new Promise(() => {}), // never resolves
    );
    render(<ModelSelector sessionId={SID} />);
    // Trigger label is the loading hint while the catalog is in flight.
    expect(screen.getByTestId("model-selector-trigger").textContent).toContain("loading");
  });

  it("renders an error when the catalog fetch fails", async () => {
    mockGetCatalog.mockRejectedValueOnce(new Error("catalog offline"));
    render(<ModelSelector sessionId={SID} />);
    await waitFor(() => {
      expect(screen.getByText("catalog offline")).toBeInTheDocument();
    });
  });
});
