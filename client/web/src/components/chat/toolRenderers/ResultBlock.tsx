// ResultBlock — render del result de un tool con status badge + body
// por tool.
//
// EP-2026-08-19: antes el body se renderizaba con `SmartResult` (JSON
// / diff / text / media). Ahora:
//
//   1. `parseStatus` del registry detecta ok/fail/empty/interrupted
//      y lo mostramos como badge (✓ ok / ✗ fail / ∅ empty).
//   2. Si el registry conoce el tool, intenta su `renderBody` custom
//      (ej. shell separa stdout/stderr, grep lista matches).
//   3. Si no, cae al `SmartResult` (que ya maneja JSON, diff, texto).
//
// Tools sin schema conocido → badge genérico + SmartResult. Nunca
// rompemos nada.

import { SmartResult } from "../SmartResult";
import { getToolConfig } from "./registry";
import type { ParsedStatus, ResultStatus } from "./types";

const STATUS_LABEL: Record<ResultStatus, string> = {
  ok: "✓ ok",
  fail: "✗ fail",
  empty: "∅ empty",
  interrupted: "⚠ interrupted",
};

interface ResultBlockProps {
  tool: string;
  output: string | undefined;
  /** True si el stream terminó sin tool_result (Fix #4). */
  interrupted?: boolean;
  /** Status ya computado por el caller (ej. cuando el output es
   *  `undefined` y queremos mostrar "interrupted"). Si no, el
   *  ResultBlock lo computa via el registry. */
  statusOverride?: ParsedStatus;
}

export function ResultBlock({
  tool,
  output,
  interrupted = false,
  statusOverride,
}: ResultBlockProps) {
  if (interrupted || output === undefined) {
    return (
      <div
        className="tool-renderer__result tool-renderer__result--interrupted"
        data-testid="tool-result-interrupted"
      >
        <span className="tool-renderer__status tool-renderer__status--interrupted">
          ⚠ interrupted (no result received)
        </span>
      </div>
    );
  }

  // Lazy require para evitar cycle con registry.tsx (que importa
  // ToolCodeBlock). El bundle split los separa igual.
  const config = getToolConfig(tool);

  const parsed = statusOverride ?? config.parseStatus?.(output) ?? null;

  const customBody = config.renderBody?.(output, parsed);
  const status: ParsedStatus | null =
    parsed ?? (output.trim().length === 0 ? { status: "empty" } : null);

  return (
    <div
      className={`tool-renderer__result${status ? ` tool-renderer__result--${status.status}` : ""}`}
      data-testid="tool-result"
      data-status={status?.status ?? "unknown"}
    >
      {status && (
        <div
          className={`tool-renderer__status tool-renderer__status--${status.status}`}
          data-testid="tool-result-status"
        >
          <span className="tool-renderer__status-label">
            {STATUS_LABEL[status.status]}
          </span>
          {status.code !== undefined && (
            <span className="tool-renderer__status-code">
              {status.code}
            </span>
          )}
          {status.message && (
            <span className="tool-renderer__status-message" title={status.message}>
              {status.message}
            </span>
          )}
        </div>
      )}
      <div className="tool-renderer__result-body">
        {customBody ?? (
          // Fallback: SmartResult (JSON, diff, texto).
          <SmartResult output={output} />
        )}
      </div>
    </div>
  );
}
