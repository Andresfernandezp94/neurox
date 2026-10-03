// StreamChunk — el contrato wire del streaming endpoint.
//
// Hasta ahora los tipos de chunks que `ChatPanel.handleSend` conocía
// vivían implícitos en un `as { type?: string; ... } | null`. Resultado:
// bugs silenciosos (chunks sin `type` se trataban como content,
// chunks con campos faltantes se aceptaban). Este archivo formaliza
// el contrato y centraliza el parser.
//
// Cualquier chunk que no matchee uno de los shapes conocidos devuelve
// `null` desde `parseStreamChunk` — el caller lo ignora en silencio
// (mejor que mezclar campos sueltos con el content del agente).
//
// Backend event shapes:
//
//   {"type":"thinking",       "text":"...", "seq":N}
//   {"type":"content",        "text":"...", "seq":N}
//   {"type":"error",          "message":"..."}
//   {"type":"tool_call",      "tool":"...", "args":{...}, "iteration":N, "call_id":"...", "seq":N}
//   {"type":"tool_result",    "tool":"...", "result":"...", "iteration":N, "call_id":"...", "seq":N}
//   {"type":"approval_request","id":"...", "tool":"...", "args":{...},
//                                  "reason":"..."}
//   {"type":"approval_resolved","id":"...", "decision":"approve"|"deny"}
//   "[DONE]"
//
// `seq` (PARTE A) is a monotonic per-session u64 the daemon assigns to
// every stream chunk. The frontend uses it to apply each chunk exactly
// once, in order, regardless of how many transport paths deliver it.

export type StreamChunk =
  | ThinkingChunk
  | ContentChunk
  | ToolCallChunk
  | ToolResultChunk
  | ApprovalRequestChunk
  | ApprovalResolvedChunk
  | ErrorChunk;

export interface ThinkingChunk {
  type: "thinking";
  text: string;
  /** Monotonic per-session sequence number assigned by the daemon
   *  (PARTE A). Optional so legacy chunks without it still parse; the
   *  ordering guard in `useChatTabs` ignores chunks without a `seq`. */
  seq?: number;
}

export interface ContentChunk {
  type: "content";
  text: string;
  /** Monotonic per-session sequence number (see `ThinkingChunk.seq`). */
  seq?: number;
}

/** The LLM invoked a tool. Captured into the timeline as a pending
 *  tool entry; the matching `ToolResultChunk` fills its `result`. */
export interface ToolCallChunk {
  type: "tool_call";
  tool: string;
  args?: unknown;
  iteration?: number;
  /** EP-2026-10-03: id de ESTA llamada, el mismo que genera el modelo.
   *  Viaja tanto en el `tool_call` como en su `tool_result`, y es lo
   *  unico que empareja una llamada con su resultado de forma fiable: con
   *  dos tools iguales en el mismo turno, `tool` + `iteration` no las
   *  distinguen. Opcional por compatibilidad con daemons antiguos; si
   *  falta se cae al emparejado por posicion de siempre. */
  call_id?: string;
  /** Monotonic per-session sequence number (see `ThinkingChunk.seq`). */
  seq?: number;
}

/** The tool finished. Either fills the matching pending entry in the
 *  timeline (same `tool`, no result yet) or — if there's no match — is
 *  pushed as a standalone entry so the data isn't lost. */
export interface ToolResultChunk {
  type: "tool_result";
  tool: string;
  result: string;
  iteration?: number;
  /** EP-2026-10-03: ver `ToolCallChunk.call_id`. */
  call_id?: string;
  /** Monotonic per-session sequence number (see `ThinkingChunk.seq`). */
  seq?: number;
}

/** The agent paused to ask for human approval on a potentially
 *  destructive tool. UI shows approve/deny buttons. */
export interface ApprovalRequestChunk {
  type: "approval_request";
  id: string;
  tool: string;
  args?: unknown;
  reason?: string;
}

/** The user (or daemon) resolved an outstanding approval. Decision
 *  is reflected on the matching approval entry in the timeline. */
export interface ApprovalResolvedChunk {
  type: "approval_resolved";
  id: string;
  decision: "approve" | "deny";
}

/** Backend reported an error mid-stream. Rendered as a banner; the
 *  assistant message is closed out with whatever text had accumulated. */
export interface ErrorChunk {
  type: "error";
  message: string;
}

/**
 * Type-narrow a raw value from the SSE stream into a `StreamChunk`.
 * Returns `null` for:
 *  - non-objects (null, primitives, arrays)
 *  - objects without a recognised `type` field
 *  - objects of a recognised type but missing their required field(s)
 *
 * Behaviour notes:
 *  - Empty/whitespace-only `text` on a thinking/content chunk is still
 *    accepted — the reducer decides whether to drop the resulting entry.
 *  - `tool_result.result` must be a string (per current wire format).
 *    A non-string `result` is rejected, not coerced.
 *  - `approval_resolved.decision` is validated against the two
 *    known values; anything else is rejected.
 */
export function parseStreamChunk(raw: unknown): StreamChunk | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const o = raw as Record<string, unknown>;

  // The daemon (PARTE A) tags every stream event with a monotonic
  // per-session `seq: u64`. It arrives as a plain JSON number. Legacy
  // events without it parse fine — `seq` stays `undefined` and the
  // ordering guard in `useChatTabs` ignores them.
  const seq = typeof o.seq === "number" ? o.seq : undefined;

  switch (o.type) {
    case "thinking": {
      if (typeof o.text !== "string") return null;
      return { type: "thinking", text: o.text, seq };
    }
    case "content": {
      if (typeof o.text !== "string") return null;
      return { type: "content", text: o.text, seq };
    }
    case "tool_call": {
      if (typeof o.tool !== "string" || !o.tool) return null;
      return {
        type: "tool_call",
        tool: o.tool,
        args: o.args,
        iteration: typeof o.iteration === "number" ? o.iteration : undefined,
        call_id: typeof o.call_id === "string" && o.call_id ? o.call_id : undefined,
        seq,
      };
    }
    case "tool_result": {
      if (typeof o.tool !== "string" || !o.tool) return null;
      if (typeof o.result !== "string") return null;
      return {
        type: "tool_result",
        tool: o.tool,
        result: o.result,
        iteration: typeof o.iteration === "number" ? o.iteration : undefined,
        call_id: typeof o.call_id === "string" && o.call_id ? o.call_id : undefined,
        seq,
      };
    }
    case "approval_request": {
      if (typeof o.id !== "string" || !o.id) return null;
      if (typeof o.tool !== "string" || !o.tool) return null;
      return {
        type: "approval_request",
        id: o.id,
        tool: o.tool,
        args: o.args,
        reason: typeof o.reason === "string" ? o.reason : undefined,
      };
    }
    case "approval_resolved": {
      if (typeof o.id !== "string" || !o.id) return null;
      if (o.decision !== "approve" && o.decision !== "deny") return null;
      return { type: "approval_resolved", id: o.id, decision: o.decision };
    }
    case "error": {
      if (typeof o.message !== "string") return null;
      return { type: "error", message: o.message };
    }
    default:
      // Unknown `type` (or missing). Per the contract, ignored — the
      // previous behaviour of treating type-less chunks as content was
      // the source of the "tool result leaking into agent reply" bug.
      return null;
  }
}
