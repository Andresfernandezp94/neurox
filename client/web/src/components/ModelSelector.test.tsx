// EP-0024: ModelSelector is a custom dropdown (button + listbox), not
// a <select> with <optgroup>. These tests were rewritten to drive
// the dropdown UI (click trigger → assert listbox items).

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ModelSelector } from "./ModelSelector";

vi.mock("../api/llm", () => ({
  getProviders: vi.fn(),
  listModels: vi.fn(),
  setSessionModel: vi.fn(),
}));

import { getProviders, listModels, setSessionModel } from "../api/llm";

const mockGetProviders = getProviders as ReturnType<typeof vi.fn>;
const mockListModels = listModels as ReturnType<typeof vi.fn>;
const mockSetSessionModel = setSessionModel as ReturnType<typeof vi.fn>;

const SID = "test-session-123";

// Per-provider discovered models (shape returned by /v1/llm/providers/:id/models).
const discoveredByProvider: Record<string, Array<{
  id: string;
  capability?: string;
  supports_tools?: boolean;
  is_free?: boolean;
}>> = {
  minimax: [
    { id: "MiniMax-M3", capability: "text-generation", supports_tools: true },
    { id: "MiniMax-M2.7", capability: "text-generation", supports_tools: true },
  ],
  openai: [
    { id: "gpt-4o", capability: "text-generation", supports_tools: true },
    { id: "gpt-4o-mini", capability: "text-generation", supports_tools: true },
  ],
  openrouter: [
    { id: "gpt-4o", capability: "text-generation", supports_tools: false },
  ],
};

const providersResp = {
  providers: [
    {
      id: "minimax",
      kind: "minimax",
      model: "MiniMax-M3",
      base_url: "https://api.minimaxi.chat/v1",
      configured: true,
      active: true,
    },
    {
      id: "openai",
      kind: "openai_compat",
      model: "gpt-4o",
      base_url: "https://api.openai.com/v1",
      configured: true,
      active: false,
    },
    {
      id: "openrouter",
      kind: "openai_compat",
      model: "liquid/lfm-2.5-2.6b:free",
      base_url: "https://openrouter.ai/api/v1",
      configured: true,
      active: false,
    },
  ],
  default_provider: "minimax",
  default_model: "MiniMax-M3",
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
    mockGetProviders.mockResolvedValue(providersResp);
    mockListModels.mockImplementation(async (providerId: string) => {
      return discoveredByProvider[providerId] ?? [];
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
  it("shows 'select model' placeholder when no currentModel is provided", async () => {
    render(<ModelSelector sessionId={SID} />);
    // Catalog loads in the background; the trigger text is "loading…"
    // initially, then "select model" once providersResp resolves.
    await waitFor(() => {
      const trigger = screen.getByTestId("model-selector-trigger");
      expect(trigger.textContent).toContain("select model");
      expect(trigger.textContent).not.toContain("MiniMax M3");
    });
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
      expect(trigger.textContent).toContain("gpt 4o");
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
      expect(screen.getByTestId("model-selector-trigger").textContent).toContain("gpt 4o");
    });
  });

  it("reverts to the previous selection when PUT fails", async () => {
    mockSetSessionModel.mockRejectedValueOnce(new Error("backend boom"));
    render(<ModelSelector sessionId={SID} currentModel={{ provider_id: "minimax", model: "MiniMax-M3" }} />);
    await waitFor(() => {
      expect(screen.getByTestId("model-selector-trigger").textContent).toContain("MiniMax M3");
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
    expect(screen.getByTestId("model-selector-trigger").textContent).toContain("MiniMax M3");
  });

  it("renders a loading hint before the catalog arrives", () => {
    mockGetProviders.mockImplementation(
      () => new Promise(() => {}), // never resolves
    );
    render(<ModelSelector sessionId={SID} />);
    // Trigger label is the loading hint while the catalog is in flight.
    expect(screen.getByTestId("model-selector-trigger").textContent).toContain("loading");
  });

  it("renders an error when the providers fetch fails", async () => {
    mockGetProviders.mockRejectedValueOnce(new Error("providers offline"));
    render(<ModelSelector sessionId={SID} />);
    await waitFor(() => {
      expect(screen.getByText("providers offline")).toBeInTheDocument();
    });
  });
});
