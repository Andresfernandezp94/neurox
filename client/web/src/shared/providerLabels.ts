// providerLabels.ts — nombre de marca a mostrar por provider.
//
// El daemon no manda un campo de display: `LlmProviderStatus` sólo trae
// `id` (`"minimax"`) y `kind`. Mostrar el id crudo produce títulos como
// "minimax" o "openai", que no son los nombres con que se conocen las
// marcas. Acá se mapea id/kind → nombre correcto.
//
// El fallback invierte el caso del id para providers nuevos que no
// estén en la tabla, así un provider agregado a mano no sale en blanco.

const BY_ID: Record<string, string> = {
  minimax: "MiniMax",
  openrouter: "OpenRouter",
  openai: "OpenAI",
  // La marca se escribe en minúscula (opencode), no capitalizada.
  opencode: "opencode",
  claude: "Claude",
  anthropic: "Anthropic",
  mistral: "Mistral",
  grok: "Grok",
  xai: "xAI",
  meta: "Meta",
  llama: "Llama",
  google: "Google",
  gemini: "Gemini",
  ollama: "Ollama",
  lmstudio: "LM Studio",
  together: "Together",
  deepseek: "DeepSeek",
};

const BY_KIND: Record<string, string> = {
  minimax: "MiniMax",
  anthropic: "Anthropic",
  openai_compat: "OpenAI-compatible",
};

export function providerDisplayName(id: string, kind?: string): string {
  const key = id.trim().toLowerCase();

  const direct = BY_ID[key];
  if (direct) return direct;

  // Providers locales ANTES del match por infijo: `local-llama` contiene
  // "llama", así que si el infijo corriera primero saldría "Llama" y se
  // perdería que es local. No basta con devolver "Local" para todos —
  // local-llama, local-ollama y local-broken quedarían indistinguibles,
  // así que se conserva el sufijo.
  if (key === "local" || kind === "local") return "Local";
  if (key.startsWith("local-")) {
    const rest = key.slice("local-".length);
    return rest ? `Local · ${rest}` : "Local";
  }
  if (key.endsWith("-local")) {
    const rest = key.slice(0, -"-local".length);
    return rest ? `Local · ${rest}` : "Local";
  }

  // Prefijos e infijos: `anthropic-pro`, `mistral-large`,
  // `openrouter/auto`.
  for (const [needle, label] of Object.entries(BY_ID)) {
    if (key.includes(needle)) return label;
  }

  if (kind && BY_KIND[kind]) return BY_KIND[kind];

  // Fallback: capitaliza la primera letra. Mejor que el id crudo en
  // minúsculas, que es como llegaba antes.
  return id.charAt(0).toUpperCase() + id.slice(1);
}