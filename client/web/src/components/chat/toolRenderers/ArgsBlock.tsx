// ArgsBlock — renderiza los args de un tool con labels legibles.
//
// EP-2026-08-19: antes se mostraba como JSON crudo. Ahora:
//
//   1. Para cada key conocida del tool (path, cmd, query, limit, …)
//      muestra un par label:valor con el formato adecuado (path
//      monospace, code inline, número, boolean, duración).
//   2. Para keys desconocidas, las junta en un bloque JSON colapsable.
//
// Si `args` es `undefined` o no es un objeto, no se muestra nada (el
// wrapper del tool ya muestra un toggle vacío en ese caso).

import { useState } from "react";
import { ToolCodeBlock } from "../../../shared/components/molecules/ToolCodeBlock";
import { useToolsSchema } from "./useToolsSchema";
import { TOOL_REGISTRY } from "./registry";
import type { ArgLabelSpec } from "./types";

interface ArgsBlockProps {
  tool: string;
  args: unknown;
}

type ArgValue = string | number | boolean | null | undefined;

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v != null && typeof v === "object" && !Array.isArray(v);
}

function formatValue(value: ArgValue, spec: ArgLabelSpec): string {
  const presentation = spec.presentation ?? "text";
  if (value === null || value === undefined) return "—";
  if (presentation === "boolean") return value ? "✓" : "✗";
  if (presentation === "duration") {
    // Best-effort: si es número interpretamos como ms, si es string lo pasamos tal cual.
    if (typeof value === "number") return `${value.toLocaleString()} ms`;
    return String(value);
  }
  if (presentation === "number") {
    if (typeof value === "number") return value.toLocaleString();
    return String(value);
  }
  if (presentation === "path" || presentation === "code") {
    return String(value);
  }
  if (presentation === "multiline-code") {
    const s = String(value);
    if (s.length > 120) return `${s.slice(0, 120)}… (+${s.length - 120} chars)`;
    return s;
  }
  return String(value);
}

function shouldHide(spec: ArgLabelSpec, value: unknown): boolean {
  if (!spec.hideIfEmpty) return false;
  if (value === null || value === undefined) return true;
  if (typeof value === "string" && value.length === 0) return true;
  return false;
}

export function ArgsBlock({ tool, args }: ArgsBlockProps) {
  const [extrasOpen, setExtrasOpen] = useState(false);
  const liveSchema = useToolsSchema();
  const liveLabels = liveSchema.labelMap[tool] ?? {};

  if (!isPlainObject(args)) return null;
  const obj = args;
  const keys = Object.keys(obj);

  // Merge de labels: hardcoded (registry) gana sobre live schema.
  const labels = { ...liveLabels, ...(TOOL_REGISTRY[tool]?.argLabels ?? {}) };

  const knownKeys = keys.filter(
    (k) => k in labels && !shouldHide(labels[k]!, obj[k]),
  );
  const unknownKeys = keys.filter((k) => !(k in labels));

  // Si no hay nada conocido, mostramos el JSON entero (compatibilidad).
  if (knownKeys.length === 0) {
    return (
      <ToolCodeBlock
        code={JSON.stringify(obj, null, 2)}
        language="json"
        className="tool-renderer__args-json"
        testId="tool-args-json"
      />
    );
  }

  return (
    <div className="tool-renderer__args" data-testid="tool-args">
      <dl className="tool-renderer__args-list">
        {knownKeys.map((k) => {
          const spec = labels[k]!;
          const value = obj[k] as ArgValue;
          const presentation = spec.presentation ?? "text";
          const isMultiline = presentation === "multiline-code";
          const fullText =
            isMultiline && typeof value === "string" ? value : null;
          return (
            <div
              key={k}
              className={`tool-renderer__args-row tool-renderer__args-row--${presentation}`}
              data-testid={`tool-args-row-${k}`}
            >
              <dt className="tool-renderer__args-label">{spec.label}</dt>
              <dd className="tool-renderer__args-value">
                {fullText !== null ? (
                  <details
                    className="tool-renderer__args-multiline"
                    data-testid={`tool-args-multiline-${k}`}
                  >
                    <summary>
                      {formatValue(
                        fullText.slice(0, 120),
                        { ...spec, presentation: "text" },
                      )}
                      {fullText.length > 120 ? "…" : ""}
                    </summary>
                    <ToolCodeBlock
                      code={fullText}
                      language="text"
                      className="tool-renderer__args-multiline-body"
                    />
                  </details>
                ) : (
                  formatValue(value, spec)
                )}
              </dd>
            </div>
          );
        })}
      </dl>
      {unknownKeys.length > 0 && (
        <details
          className="tool-renderer__args-extras"
          open={extrasOpen}
          onToggle={(e) => setExtrasOpen((e.target as HTMLDetailsElement).open)}
        >
          <summary className="tool-renderer__args-extras-summary">
            + {unknownKeys.length} more param
            {unknownKeys.length === 1 ? "" : "s"}
          </summary>
          <ToolCodeBlock
            code={JSON.stringify(
              Object.fromEntries(unknownKeys.map((k) => [k, obj[k]])),
              null,
              2,
            )}
            language="json"
            className="tool-renderer__args-extras-body"
            testId="tool-args-extras"
          />
        </details>
      )}
    </div>
  );
}
