// FamilyLogo — maps HF family id to the corresponding SVG asset.
// Falls back to IconDefaultAgent when no asset matches.
//
// Conventions:
//   - phi → microsoft (reuses the Copilot mark as the closest visual
//     hint for the Microsoft family, since dedicated Phi/MAI logos are
//     not yet published).
//   - whisper → openai (the OpenAI mark covers the Whisper family).

import type { ReactNode } from "react";
import { IconDefaultAgent } from "../components/Icons";
import qwenUrl from "../assets/families/qwen-color.svg?url";
import metaUrl from "../assets/families/meta-color.svg?url";
import mistralUrl from "../assets/families/mistral-color.svg?url";
import googleUrl from "../assets/families/google-color.svg?url";
import deepseekUrl from "../assets/families/deepseek-color.svg?url";
import microsoftUrl from "../assets/families/microsoft-color.svg?url";
import openaiUrl from "../assets/families/openai-color.svg?url";

interface FamilyLogoProps {
  id: string;
  className?: string;
}

function matchLogo(id: string): string | null {
  const lower = id.toLowerCase();
  if (lower.startsWith("qwen")) return qwenUrl;
  if (lower.startsWith("llama") || lower.startsWith("meta")) return metaUrl;
  if (lower.startsWith("mistral")) return mistralUrl;
  if (lower.startsWith("gemma") || lower.startsWith("google")) return googleUrl;
  if (lower.startsWith("deepseek")) return deepseekUrl;
  if (lower.startsWith("phi") || lower.startsWith("microsoft")) return microsoftUrl;
  if (lower.startsWith("whisper") || lower.startsWith("openai")) return openaiUrl;
  return null;
}

export function FamilyLogo({ id, className }: FamilyLogoProps): ReactNode {
  const url = matchLogo(id);
  if (!url) return <IconDefaultAgent />;
  return <img src={url} alt={`${id} logo`} className={className ?? "family-logo"} />;
}
