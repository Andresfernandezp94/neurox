import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ProviderSelector } from "./ProviderSelector";

vi.mock("../api/llm", () => ({
  getProviders: vi.fn(),
  setActiveProvider: vi.fn(),
  testProvider: vi.fn(),
}));

import { getProviders, setActiveProvider } from "../api/llm";

const mockGetProviders = getProviders as ReturnType<typeof vi.fn>;
const mockSetActive = setActiveProvider as ReturnType<typeof vi.fn>;

const providers = [
  { id: "minimax", kind: "minimax", model: "MiniMax-M3", base_url: "https://api.minimaxi.chat/v1", configured: true, active: true },
  { id: "openai", kind: "openai_compat", model: "gpt-4o", base_url: "https://api.openai.com/v1", configured: true, active: false },
  { id: "anthropic", kind: "anthropic", model: "claude-3-5-haiku-latest", base_url: "https://api.anthropic.com", configured: false, active: false },
];

const providersResp = {
  providers,
  default_provider: "minimax",
  default_model: "MiniMax-M3",
};

describe("ProviderSelector", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetProviders.mockResolvedValue(providersResp);
    mockSetActive.mockResolvedValue(undefined);
  });

  it("renders with providers and shows active", async () => {
    render(<ProviderSelector />);
    await waitFor(() => {
      expect(screen.getByLabelText("LLM Provider")).toBeInTheDocument();
    });
    const select = screen.getByLabelText("LLM Provider") as HTMLSelectElement;
    expect(select.value).toBe("minimax");
  });

  it("renders unconfigured provider as disabled option", async () => {
    render(<ProviderSelector />);
    await waitFor(() => {
      expect(screen.getByLabelText("LLM Provider")).toBeInTheDocument();
    });
    const options = screen.getAllByRole("option");
    const anthropic = options.find((o) => o.textContent?.includes("anthropic"));
    expect(anthropic).toHaveAttribute("disabled");
  });

  it("calls setActiveProvider on change", async () => {
    render(<ProviderSelector />);
    await waitFor(() => {
      expect(screen.getByLabelText("LLM Provider")).toBeInTheDocument();
    });
    const select = screen.getByLabelText("LLM Provider");
    fireEvent.change(select, { target: { value: "openai" } });
    await waitFor(() => {
      expect(mockSetActive).toHaveBeenCalledWith("openai");
    });
  });

  it("shows error on API failure and preserves selection", async () => {
    mockSetActive.mockRejectedValueOnce(new Error("swap failed"));
    render(<ProviderSelector />);
    await waitFor(() => {
      expect(screen.getByLabelText("LLM Provider")).toBeInTheDocument();
    });
    const select = screen.getByLabelText("LLM Provider");
    fireEvent.change(select, { target: { value: "openai" } });
    await waitFor(() => {
      expect(screen.getByText("swap failed")).toBeInTheDocument();
    });
  });
});
