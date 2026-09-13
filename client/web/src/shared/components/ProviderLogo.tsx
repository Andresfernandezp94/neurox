// ProviderLogo — maps provider id/kind to the corresponding SVG asset.
// Falls back to IconDefaultAgent when no asset matches.

import type { ReactNode } from "react";
import { IconDefaultAgent } from "../components/Icons";
import claudeUrl from "../assets/providers/claude-color.svg?url";
import mistralUrl from "../assets/providers/mistral-color.svg?url";
import openaiUrl from "../assets/providers/openai.svg?url";
import metaUrl from "../assets/providers/meta-color.svg?url";
import googleUrl from "../assets/providers/google-color.svg?url";
import grokUrl from "../assets/providers/grok.svg?url";
import minimaxUrl from "../assets/providers/minimax-color.svg?url";
import openrouterUrl from "../assets/providers/openrouter-color.svg?url";
import ollamaUrl from "../assets/providers/ollama.svg?url";
import localUrl from "../assets/providers/local-color.svg?url";

interface ProviderLogoProps {
  id: string;
  kind?: string;
  className?: string;
}

function matchLogo(id: string, kind?: string): string | null {
  const lower = id.toLowerCase();
  // Match by id FIRST so providers like `mistral` (kind: openai_compat)
  // don't get mis-routed to the generic OpenAI logo. The kind fallback
  // at the bottom only kicks in when the id gives no hint.
  if (lower.startsWith("local-") || lower.endsWith("-local") || kind === "local") {
    return localUrl;
  }
  if (lower.startsWith("minimax")) return minimaxUrl;
  if (lower.startsWith("openrouter")) return openrouterUrl;
  if (lower.startsWith("openai")) return openaiUrl;
  if (lower.startsWith("claude")) return claudeUrl;
  if (lower.startsWith("mistral")) return mistralUrl;
  if (lower.startsWith("meta") || lower.startsWith("llama-")) return metaUrl;
  if (lower.startsWith("google") || lower.startsWith("gemini")) return googleUrl;
  if (lower.startsWith("grok")) return grokUrl;
  if (lower.startsWith("ollama")) return ollamaUrl;
  // Kind fallback (only when the id gives no hint).
  if (kind === "anthropic") return claudeUrl;
  if (kind === "openai_compat") return openaiUrl;
  return null;
}

export function ProviderLogo({ id, kind, className }: ProviderLogoProps): ReactNode {
  const url = matchLogo(id, kind);
  if (!url) return <IconDefaultAgent />;
  const lower = id.toLowerCase();
  // openai / grok assets are single-color black SVGs — render them white.
  const white =
    lower.startsWith("openai") || lower.startsWith("grok");
  const cls = `${className ?? "provider-logo"}${white ? " provider-logo--white" : ""}`;
  return <img src={url} alt={`${id} logo`} className={cls} />;
}