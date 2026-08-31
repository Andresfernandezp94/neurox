// ToolNode — one tool call in the chat timeline.
//
// EP-2026-08-19: estructura de toggle anidado:
//   ┌─ [shell · exit 0] (wrapper toggle, click expande args + result)
//   │    ├─ [→ args preview] (toggle interno)
//   │    └─ [→ result preview] (toggle interno + status badge)
//
// El wrapper mantiene colapsado por default (solo el nombre de la
// tool). Cuando se expande muestra los toggles de args y result, que
// son independientes (uno puede estar expandido sin el otro).
//
// Args y result se renderizan con el módulo `toolRenderers/`:
//   - ArgsBlock muestra los parámetros con labels legibles (path,
//     query, limit, …) en lugar de JSON crudo.
//   - ResultBlock muestra un status badge (ok/fail/empty) + body
//     específico por tool (shell separa stdout/stderr, etc.).
//
// EP-2026-08-19 fix #4: si el stream terminó sin tool_result
// (`!hasResult && !isStreaming`), mostramos un badge "interrupted"
// en lugar de un toggle de result vacío.

import { useState } from "react";
import { ArgsBlock, ResultBlock } from "./toolRenderers";
import { TOOL_REGISTRY } from "./toolRenderers/registry";
import type { ToolActivity } from "../../types";

function hasArgsObject(args: unknown): args is Record<string, unknown> {
  return (
    args != null &&
    typeof args === "object" &&
    Object.keys(args as Record<string, unknown>).length > 0
  );
}

function previewArgs(args: unknown): string {
  if (!hasArgsObject(args)) return "";
  const entries = Object.entries(args);
  if (entries.length === 0) return "";
  const [firstKey, firstVal] = entries[0]!;
  const valueStr =
    typeof firstVal === "string"
      ? firstVal
      : JSON.stringify(firstVal);
  const formatted = valueStr.length > 60 ? valueStr.slice(0, 60) + "…" : valueStr;
  const rest = entries.length - 1;
  return rest > 0 ? `${firstKey}: ${formatted} +${rest}` : `${firstKey}: ${formatted}`;
}

function previewResult(result: string | undefined, tool: string): string {
  if (!result) return "";
  // Si el registry tiene un caption custom, lo usamos (ej. "exit 0",
  // "142 bytes"). Si no, primer línea del output.
  const caption = TOOL_REGISTRY[tool]?.caption?.(result, null);
  if (caption) return caption;
  // EP-2026-08-19: para tools media (generate_image / audio / video)
  // el body va a ser un <img>/<video>/<audio>. El preview del toggle
  // debería decir algo legible ("image saved", "saved"), no el JSON
  // crudo que el backend manda (ej. `{"ok":true,"output":"...Image
  // generated..."}`). Si vemos una extensión media en el output,
  // mostramos un preview específico.
  if (
    tool === "generate_image" ||
    tool === "generate_music" ||
    tool === "generate_audio" ||
    tool === "generate_video"
  ) {
    if (/\.(png|jpg|jpeg|gif|webp|svg|mp3|wav|ogg|m4a|flac|mp4|webm|mov)/i.test(result)) {
      const kind =
        tool === "generate_image"
          ? "image"
          : tool === "generate_video"
            ? "video"
            : "audio";
      return `${kind} saved`;
    }
  }
  const trimmed = result.trim();
  const first = trimmed.split("\n")[0] ?? "";
  if (first.length > 80) return first.slice(0, 80) + "…";
  return first;
}

/**
 * Glyph identificador por tool — un solo caracter que distingue
 * cada tool visualmente. Cambiar aquí para reasignar todos los toggles.
 */
function toolGlyph(tool: string): string {
  switch (tool) {
    case "shell":
      return ">_";
    case "read_file":
      return "F";
    case "write_file":
      return "W";
    case "edit_file":
      return "E";
    case "grep":
      return "/";
    case "glob":
      return "*";
    case "list_dir":
      return "≡";
    case "web_search":
      return "S";
    case "web_fetch":
      return "↗";
    case "save_fact":
      return "+";
    case "search_memory":
      return "M";
    case "symbols":
      return "Σ";
    default:
      return "·";
  }
}

export function ToolNode({
  activity,
  isStreaming = false,
}: {
  activity: ToolActivity;
  isStreaming?: boolean;
}) {
  const [wrapperOpen, setWrapperOpen] = useState(false);
  const [argsOpen, setArgsOpen] = useState(false);
  const [resultOpen, setResultOpen] = useState(false);
  const hasResult = !!activity.result;
  const hasArgs = hasArgsObject(activity.args);
  const interrupted = !hasResult && !isStreaming;

  const argsPreview = previewArgs(activity.args);
  const resultPreview = previewResult(activity.result, activity.tool);

  // Caption del wrapper (ej. "shell · exit 0", "write_file · 142 bytes").
  const wrapperCaption =
    activity.result && !interrupted
      ? TOOL_REGISTRY[activity.tool]?.caption?.(activity.result, null) ?? null
      : null;

  return (
    <div className="chat__timeline-node chat__timeline-node--tool">
      <button
        type="button"
        className={`chat__timeline-tool-wrapper${wrapperOpen ? " chat__timeline-tool-wrapper--open" : ""}`}
        onClick={() => setWrapperOpen((o) => !o)}
        aria-expanded={wrapperOpen}
        data-testid="tool-wrapper-toggle"
      >
        <span
          className="chat__timeline-tool-wrapper-glyph"
          data-tool={activity.tool}
          aria-hidden="true"
        >
          {toolGlyph(activity.tool)}
        </span>
        <span className="chat__timeline-label chat__timeline-label--tool">
          {activity.tool}
        </span>
        {wrapperCaption && (
          <span
            className="chat__timeline-tool-wrapper-caption"
            data-testid="tool-wrapper-caption"
          >
            · {wrapperCaption}
          </span>
        )}
        {!hasResult && isStreaming && (
          <span
            className="chat__timeline-tool-loading"
            aria-label="Running"
            data-testid="tool-loading-dots"
          >
            <span className="chat__timeline-tool-loading-dot" />
            <span className="chat__timeline-tool-loading-dot" />
            <span className="chat__timeline-tool-loading-dot" />
          </span>
        )}
      </button>

      {wrapperOpen && (
        <div className="chat__timeline-tool-children">
          {hasArgs && (
            <>
              <button
                type="button"
                className={`chat__timeline-result-preview${argsOpen ? " chat__timeline-result-preview--open" : ""}`}
                onClick={() => setArgsOpen((o) => !o)}
                aria-expanded={argsOpen}
                data-testid="tool-args-toggle"
              >
                <span className="chat__timeline-result-preview-kind">args</span>
                <span className="chat__timeline-result-preview-text">
                  {argsPreview}
                </span>
              </button>
              {argsOpen && (
                <div data-testid="tool-args-body" className="chat__tool-block chat__tool-block--args">
                  <ArgsBlock tool={activity.tool} args={activity.args} />
                </div>
              )}
            </>
          )}

          {interrupted ? (
            // EP-2026-08-19 fix #4: stream ended without a tool_result.
            <ResultBlock
              tool={activity.tool}
              output={undefined}
              interrupted
            />
          ) : hasResult ? (
            <>
              <button
                type="button"
                className={`chat__timeline-result-preview${resultOpen ? " chat__timeline-result-preview--open" : ""}`}
                onClick={() => setResultOpen((o) => !o)}
                aria-expanded={resultOpen}
                data-testid="tool-result-toggle"
              >
                <span className="chat__timeline-result-preview-kind">result</span>
                <span className="chat__timeline-result-preview-text">
                  {resultPreview}
                </span>
              </button>
              {resultOpen && (
                <div data-testid="tool-result-body" className="chat__tool-block chat__tool-block--result">
                  <ResultBlock
                    tool={activity.tool}
                    output={activity.result}
                  />
                </div>
              )}
            </>
          ) : isStreaming ? (
            <button
              type="button"
              className="chat__timeline-result-preview chat__timeline-result-preview--pending"
              disabled
              aria-expanded={false}
              data-testid="tool-result-toggle"
            >
              running…
            </button>
          ) : null}
        </div>
      )}
    </div>
  );
}
