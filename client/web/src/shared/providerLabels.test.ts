import { describe, it, expect } from "vitest";
import { providerDisplayName } from "./providerLabels";

// El daemon no manda campo de display: `LlmProviderStatus` sólo trae `id`
// y `kind`. Estos tests fijan el mapeo a nombre de marca, que es lo que
// evita que los títulos salgan como "minimax" en vez de "MiniMax".
describe("providerDisplayName", () => {
  it("maps the known brands to their proper casing", () => {
    expect(providerDisplayName("minimax")).toBe("MiniMax");
    expect(providerDisplayName("openrouter")).toBe("OpenRouter");
    expect(providerDisplayName("openai")).toBe("OpenAI");
    expect(providerDisplayName("mistral")).toBe("Mistral");
    expect(providerDisplayName("anthropic")).toBe("Anthropic");
    expect(providerDisplayName("grok")).toBe("Grok");
  });

  it("keeps opencode in lowercase — that is the brand's own casing", () => {
    expect(providerDisplayName("opencode")).toBe("opencode");
  });

  it("matches on prefixes and infix", () => {
    expect(providerDisplayName("minimax-m2")).toBe("MiniMax");
    expect(providerDisplayName("mistral-large")).toBe("Mistral");
    expect(providerDisplayName("anthropic-pro")).toBe("Anthropic");
    expect(providerDisplayName("opencode/auto")).toBe("opencode");
  });

  it("is case-insensitive on the id", () => {
    expect(providerDisplayName("MiniMax")).toBe("MiniMax");
    expect(providerDisplayName("OpenRouter")).toBe("OpenRouter");
  });

  it("falls back to the kind when the id gives no hint", () => {
    expect(providerDisplayName("custom-llm", "anthropic")).toBe("Anthropic");
    expect(providerDisplayName("custom-llm", "openai_compat")).toBe(
      "OpenAI-compatible",
    );
    expect(providerDisplayName("custom-llm", "minimax")).toBe("MiniMax");
  });

  it("keeps the suffix on local providers so they stay distinguishable", () => {
    // Si devolviera "Local" a secas, local-llama, local-ollama y
    // local-broken quedarían idénticos en la lista de providers.
    expect(providerDisplayName("local-llama")).toBe("Local · llama");
    expect(providerDisplayName("llama-local")).toBe("Local · llama");
    expect(providerDisplayName("local")).toBe("Local");
    expect(providerDisplayName("anything", "local")).toBe("Local");
  });

  it("capitalizes unknown ids instead of leaving them lowercase", () => {
    // Un provider nuevo agregado a mano no debe salir como "mi-provider".
    expect(providerDisplayName("mi-provider")).toBe("Mi-provider");
  });

  it("prefers the id over the kind", () => {
    // `mistral` es openai_compat como kind, pero el id manda: si no,
    // saldría "OpenAI-compatible" en vez de "Mistral".
    expect(providerDisplayName("mistral", "openai_compat")).toBe("Mistral");
  });
});