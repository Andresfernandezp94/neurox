// useToolsSchema — pide el schema vivo de /v1/tools y devuelve un mapa
// de labels (key → ArgLabelSpec) por tool. Se mergea con los labels
// hardcoded de `registry.tsx` en `ArgsBlock` (los hardcoded ganan).
//
// EP-2026-08-19: si el daemon agrega un parámetro nuevo a un tool
// conocido, esta capa lo trae sin redeploy del frontend. Si el
// endpoint falla o está vacío, simplemente no aporta nada.
//
// Cacheamos el resultado en memoria (singleton) — el schema cambia
// muy rara vez (al recargar el daemon). Un refetch se puede disparar
// manualmente vía `invalidateToolsSchema()`.

import { useEffect, useState } from "react";
import { listTools } from "../../../api/tools";
import type { ArgLabelSpec, ArgLabels } from "./types";

interface ToolsSchemaState {
  /** tool name → key → label spec */
  labelMap: Record<string, ArgLabels>;
  /** ts del último fetch exitoso (null si nunca). */
  loadedAt: number | null;
}

const EMPTY_STATE: ToolsSchemaState = { labelMap: {}, loadedAt: null };

let cache: ToolsSchemaState = EMPTY_STATE;
let inflight: Promise<void> | null = null;

function labelFromDescription(key: string, description: string): string {
  // description suele venir como "Path to the file to read". Tomamos
  // la primera palabra capitalizada y la usamos como label. Es mejor
  // que mostrar el `snake_case` crudo.
  const trimmed = description.trim();
  if (!trimmed) return key;
  // Primera palabra significativa (letra inicial mayúscula).
  const firstWord = trimmed.split(/\s+/)[0]!;
  // Si es CamelCase o snake_case crudo, lo humanizamos mínimo.
  if (firstWord === firstWord.toUpperCase()) return firstWord;
  return firstWord.charAt(0).toUpperCase() + firstWord.slice(1);
}

async function loadOnce(): Promise<void> {
  if (inflight) return inflight;
  inflight = (async () => {
    try {
      const resp = await listTools();
      const labelMap: Record<string, ArgLabels> = {};
      for (const tool of resp.tools) {
        const props = (
          tool.parameters as { properties?: Record<string, unknown> } | undefined
        )?.properties;
        if (!props) continue;
        const labels: ArgLabels = {};
        for (const [key, rawSpec] of Object.entries(props)) {
          if (!rawSpec || typeof rawSpec !== "object") continue;
          const spec = rawSpec as { description?: unknown };
          const description =
            typeof spec.description === "string" ? spec.description : key;
          const labelSpec: ArgLabelSpec = { label: labelFromDescription(key, description) };
          // Si el nombre sugiere un tipo conocido, lo mapeamos.
          const lc = key.toLowerCase();
          if (/(path|file|dir|cwd)$/.test(lc)) labelSpec.presentation = "path";
          else if (/(code|cmd|command|pattern|query|prompt|script)$/.test(lc))
            labelSpec.presentation = "code";
          else if (/(content|body|text|new_string|old_string)$/.test(lc))
            labelSpec.presentation = "multiline-code";
          else if (/(limit|offset|max|min|count|depth|width|height|size|bytes|lines)$/.test(lc))
            labelSpec.presentation = "number";
          else if (/timeout|duration/.test(lc)) labelSpec.presentation = "duration";
          else if (/^is_|has_|show_|case_|replace_/.test(lc))
            labelSpec.presentation = "boolean";
          labels[key] = labelSpec;
        }
        labelMap[tool.name] = labels;
      }
      cache = { labelMap, loadedAt: Date.now() };
    } catch {
      // Silencioso: si falla, ArgsBlock sigue funcionando solo con
      // los labels hardcoded.
      if (cache.loadedAt === null) cache = EMPTY_STATE;
    } finally {
      inflight = null;
    }
  })();
  return inflight;
}

export function invalidateToolsSchema(): void {
  cache = EMPTY_STATE;
  inflight = null;
}

/** Hook: trae el schema vivo la primera vez que se llama. Devuelve
 *  el cache actual + un flag `loaded` para que el caller sepa si ya
 *  hubo un fetch (incluso si fue fallido). */
export function useToolsSchema(): ToolsSchemaState & { loaded: boolean } {
  const [, force] = useState(0);
  useEffect(() => {
    let cancelled = false;
    if (!inflight && cache.loadedAt === null) {
      void loadOnce().then(() => {
        if (!cancelled) force((n) => n + 1);
      });
    }
    return () => {
      cancelled = true;
    };
  }, []);
  return { ...cache, loaded: cache.loadedAt !== null };
}
