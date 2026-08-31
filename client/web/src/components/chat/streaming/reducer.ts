// streamReducer — pure state machine for applying streaming chunks
// to the assistant message.
//
// Was inlined in `ChatPanel.handleSend` (~120 lines of imperative logic
// with local `let` accumulators and ad-hoc `timeline` array mutations).
// Now extracted so:
//
//   - the reducer is pure and unit-testable (Fase 5 will add tests)
//   - the type-narrowing happens upstream in `parseStreamChunk`; the
//     reducer sees only validated `StreamChunk` values
//   - the word-boundary fix ("pegar palabras sin espacio") lives in
//     ONE place
//
// State shape:
//
//   StreamState {
//     content:   running concatenation of every ContentChunk
//     thinking:  running concatenation of every ThinkingChunk
//     toolLog:   array of { tool, args, result?, iteration } in arrival order
//     approvals: array of { id, tool, args?, reason?, decision? } in arrival order
//     timeline:  chronological list of TimelineEntry — multiple chunks
//               of the same kind collapse into the previous entry of
//               that kind (so a long stream of content chunks is one
//               growing entry, not dozens of small ones).
//   }

import type { StreamChunk } from "./chunk";
import type {
  ApprovalActivity,
  TimelineEntry,
  ToolActivity,
} from "../../../types";

export interface StreamState {
  content: string;
  thinking: string;
  toolLog: ToolActivity[];
  approvals: ApprovalActivity[];
  timeline: TimelineEntry[];
  /**
   * EP-2026-08-19: backend `{"type":"error","message":"..."}` chunks
   * used to be silently dropped (`return state` in the reducer),
   * which made the agent look hung when a tool failed. We now
   * surface the latest error message here so the consumer (ChatPanel)
   * can mirror it to its own error banner via `onState`.
   *
   * `null` = no error chunk has arrived since the stream started.
   * `setError(null)` is the consumer's job on the next `send`.
   */
  error: string | null;
  /**
   * EP-2026-08-15 (defensive): the chat backend sometimes embeds
   * a tool's stdout into the next `content` chunk instead of
   * emitting a proper `tool_result`. We track the last non-content
   * event so the reducer can detect that pattern and redirect the
   * output to the matching pending tool instead of letting it
   * pollute `state.content` (the agent's reply).
   *
   * - `null`: nothing received yet, or the last event was a
   *   `content` chunk (the LLM is talking normally).
   * - `"tool"`: last event was a `tool_call` or `tool_result` —
   *   a `content` chunk in this state is suspicious and is
   *   redirected (see the `case "content"` branch).
   * - `"approval"`: last event was an approval. A subsequent
   *   `content` chunk is treated as normal agent text (the LLM
   *   may comment right after an approval).
   */
  lastNonContentEvent: null | "tool" | "approval";
  /** Tool name when `lastNonContentEvent === "tool"`. */
  lastToolName: string | null;
}

export const initStreamState = (): StreamState => ({
  content: "",
  thinking: "",
  toolLog: [],
  approvals: [],
  timeline: [],
  error: null,
  lastNonContentEvent: null,
  lastToolName: null,
});

/**
 * Append `text` to the last entry of the given kind in `timeline`,
 * or push a new entry if the last one is a different kind. Multiple
 * chunks of the same kind collapse into a single growing entry.
 */
function appendToTimeline(
  timeline: TimelineEntry[],
  kind: "thinking" | "content",
  text: string,
): TimelineEntry[] {
  const last = timeline[timeline.length - 1];
  if (last && last.type === kind) {
    // Narrow so TS keeps the exact { type, text } shape across the
    // spread (otherwise the union widens).
    if (last.type === kind) {
      return [...timeline.slice(0, -1), { ...last, text: last.text + text }];
    }
  }
  return [...timeline, { type: kind, text }];
}

/**
 * Concatenación directa sin insertar espacios. El LLM manda el
 * texto con su propio spacing; no debemos alterarlo.
 */
// (joinChunk removed — concatenamos directo con `state.content += chunk.text`)

/**
 * Pure: apply a single validated `StreamChunk` to the state and return
 * the next state. Always returns a NEW state (immutable updates) so the
 * caller can pass the result to React's setState / reducer.
 *
 * ErrorChunk does NOT mutate the stream accumulators — it surfaces to
 * the caller for separate error rendering. (`streamMessage` catches it
 * in its own try/catch; the reducer doesn't need to act on it.)
 */
export function streamReducer(
  state: StreamState,
  chunk: StreamChunk,
): StreamState {
  switch (chunk.type) {
    case "thinking": {
      const text = chunk.text;
      // Append a new thinking entry (or merge with the last one if
      // the last entry is also thinking). This is the original
      // appendToTimeline behavior — no timestamps.
      const nextTimeline = appendToTimeline(
        state.timeline,
        "thinking",
        text,
      );
      return {
        ...state,
        thinking: state.thinking + text,
        timeline: nextTimeline,
        // EP-2026-08-15 (defensive): a thinking chunk resets the
        // "tool-output-belongs-to-next-content" tracker. Any
        // following content chunks are the LLM's own reply text.
        lastNonContentEvent: null,
        lastToolName: null,
      };
    }

    case "content": {
      // EP-2026-08-15 (defensive): si el último evento no-content
      // fue un `tool_call`, redirigimos SOLO ESTE PRIMER chunk al
      // `result` del tool. Esto captura el wire-format quirk del
      // backend donde mete el output de la tool en un chunk
      // `content` (típicamente 1 chunk, a veces más).
      //
      // Después del primer chunk redirigido volvemos a modo normal,
      // para que cualquier respuesta posterior del LLM
      // (comentario legítimo, summary, cierre) vaya al
      // `state.content`. Si el LLM va a hablar antes/después del
      // output, lo hace vía un `thinking` chunk, que resetea el
      // modo "tool" desde antes.
      //
      // Al redirigir el primer content a un tool, marcamos `endedAt`
      // en el último thinking segment (thinking terminado).
      if (
        state.lastNonContentEvent === "tool" &&
        state.lastToolName !== null
      ) {
        const toolName = state.lastToolName;
        // Resolve the iteration from the matching toolLog entry, which
        // is the source of truth for what was just called. Require an
        // exact match — a tool entry without iteration (legacy) is
        // intentionally NOT picked up here so we don't accidentally
        // redirect into an old entry from a previous turn.
        const lastEntry = state.toolLog[state.toolLog.length - 1];
        const lastIteration =
          lastEntry && lastEntry.tool === toolName ? lastEntry.iteration : undefined;
        let matchedTimelineIdx = -1;
        if (lastIteration !== undefined) {
          for (let i = state.timeline.length - 1; i >= 0; i--) {
            const e = state.timeline[i];
            if (
              e &&
              e.type === "tool" &&
              e.tool === toolName &&
              e.iteration === lastIteration
            ) {
              matchedTimelineIdx = i;
              break;
            }
          }
        }
        if (matchedTimelineIdx >= 0) {
          const matched = state.timeline[matchedTimelineIdx]!;
          if (matched.type === "tool") {
            const prevResult = matched.result ?? "";
            const nextTimeline = state.timeline.map((e, i) =>
              i === matchedTimelineIdx
                ? { ...e, result: prevResult + chunk.text }
                : e,
            );
            // Match toolLog on the same (tool, iteration) as the
            // timeline entry — otherwise a content chunk redirected
            // after a second `shell` call would append the output to
            // BOTH pending shells in toolLog.
            const nextToolLog = state.toolLog.map((t) =>
              t.tool === toolName && t.iteration === lastIteration
                ? { ...t, result: (t.result ?? "") + chunk.text }
                : t,
            );
            // Reset to "idle" (null) so subsequent content chunks
            // go to state.content as the LLM's response.
            return {
              ...state,
              timeline: nextTimeline,
              toolLog: nextToolLog,
              lastNonContentEvent: null,
              lastToolName: null,
            };
          }
        }
        // No matching tool — fall through to normal content path.
      }
      const text = chunk.text;
      return {
        ...state,
        content: state.content + text,
        timeline: appendToTimeline(state.timeline, "content", text),
        // Reset (might be a no-op if already null).
        lastNonContentEvent: null,
        lastToolName: null,
      };
    }

    case "tool_call": {
      const iteration = chunk.iteration ?? 0;
      const activity: ToolActivity = {
        tool: chunk.tool,
        args: chunk.args,
        iteration,
      };
      const timelineEntry: TimelineEntry = {
        type: "tool",
        tool: chunk.tool,
        args: chunk.args,
        iteration,
      };
      return {
        ...state,
        toolLog: [...state.toolLog, activity],
        timeline: [...state.timeline, timelineEntry],
        lastNonContentEvent: "tool",
        lastToolName: chunk.tool,
      };
    }

    case "tool_result": {
      const iteration = chunk.iteration ?? 0;

      // EP-2026-08-19: previously matched by tool name only, which
      // latched the result onto the LAST entry with that name. When
      // the agent ran two `shell` calls in a row, the first one's
      // result overwrote (or was overwritten by) the second, leaving
      // the other pending forever in the UI — the symptom that
      // looked like "the agent hung". Now we match by iteration too:
      //
      //   - Primary: same tool AND same iteration
      //   - Fallback (legacy backends that omit iteration): match
      //     by tool name only against the most recent entry
      let matchedTimelineIdx = -1;
      if (chunk.iteration !== undefined) {
        for (let i = state.timeline.length - 1; i >= 0; i--) {
          const e = state.timeline[i];
          if (
            e &&
            e.type === "tool" &&
            e.tool === chunk.tool &&
            e.iteration === chunk.iteration
          ) {
            matchedTimelineIdx = i;
            break;
          }
        }
      }
      if (matchedTimelineIdx < 0) {
        for (let i = state.timeline.length - 1; i >= 0; i--) {
          const e = state.timeline[i];
          if (e && e.type === "tool" && e.tool === chunk.tool) {
            matchedTimelineIdx = i;
            break;
          }
        }
      }
      const nextTimeline =
        matchedTimelineIdx >= 0
          ? state.timeline.map((e, i) => {
              if (i !== matchedTimelineIdx) return e;
              if (e.type === "tool") {
                return { ...e, result: chunk.result };
              }
              return e;
            })
          : [
              ...state.timeline,
              {
                type: "tool" as const,
                tool: chunk.tool,
                result: chunk.result,
                iteration,
              },
            ];

      // Mirror into toolLog with the same iteration-aware matching.
      let matchedLogIdx = -1;
      if (chunk.iteration !== undefined) {
        for (let i = state.toolLog.length - 1; i >= 0; i--) {
          const t = state.toolLog[i];
          if (t && t.tool === chunk.tool && t.iteration === chunk.iteration) {
            matchedLogIdx = i;
            break;
          }
        }
      }
      if (matchedLogIdx < 0) {
        for (let i = state.toolLog.length - 1; i >= 0; i--) {
          if (state.toolLog[i]!.tool === chunk.tool) {
            matchedLogIdx = i;
            break;
          }
        }
      }
      const nextToolLog =
        matchedLogIdx >= 0
          ? state.toolLog.map((t, i) =>
              i === matchedLogIdx ? { ...t, result: chunk.result } : t,
            )
          : [
              ...state.toolLog,
              { tool: chunk.tool, result: chunk.result, iteration },
            ];

      return {
        ...state,
        timeline: nextTimeline,
        toolLog: nextToolLog,
        // EP-2026-08-15 (defensive): a legitimate `tool_result`
        // resets the "tool-pending-redirect" mode. From here on,
        // any subsequent `content` chunks are the LLM commenting
        // on the result (wire format is correct from this point).
        lastNonContentEvent: null,
        lastToolName: null,
      };
    }

    case "approval_request": {
      const entry: ApprovalActivity = {
        id: chunk.id,
        tool: chunk.tool,
        args: chunk.args,
        reason: chunk.reason,
      };
      const timelineEntry: TimelineEntry = {
        type: "approval",
        id: chunk.id,
        tool: chunk.tool,
        args: chunk.args,
        reason: chunk.reason,
      };
      return {
        ...state,
        approvals: [...state.approvals, entry],
        timeline: [...state.timeline, timelineEntry],
        lastNonContentEvent: "approval",
        lastToolName: null,
      };
    }

    case "approval_resolved": {
      // 1. Update the timeline entry: find most-recent matching
      // pending approval in the timeline and set its decision.
      let matchedTimelineIdx = -1;
      for (let i = state.timeline.length - 1; i >= 0; i--) {
        const e = state.timeline[i];
        if (
          e &&
          e.type === "approval" &&
          e.id === chunk.id &&
          e.decision === undefined
        ) {
          matchedTimelineIdx = i;
          break;
        }
      }
      const nextTimeline =
        matchedTimelineIdx >= 0
          ? state.timeline.map((e, i) => {
              if (i !== matchedTimelineIdx) return e;
              if (e.type === "approval") {
                return { ...e, decision: chunk.decision };
              }
              return e;
            })
          : state.timeline;

      // 2. Mirror into approvals array.
      const matchedLogIdx = state.approvals.findIndex(
        (a) => a.id === chunk.id && a.decision === undefined,
      );
      const nextApprovals =
        matchedLogIdx >= 0
          ? state.approvals.map((a, i) =>
              i === matchedLogIdx ? { ...a, decision: chunk.decision } : a,
            )
          : state.approvals;

      return {
        ...state,
        timeline: nextTimeline,
        approvals: nextApprovals,
        lastNonContentEvent: "approval",
        lastToolName: null,
      };
    }

    case "error":
      // EP-2026-08-19: previously dropped silently (`return state`),
      // which made the agent look hung whenever the backend reported
      // an error mid-stream (e.g. shell exited non-zero). Surface
      // the message on StreamState so the consumer can mirror it
      // to its error banner. Don't touch the accumulators — partial
      // assistant text stays intact for context.
      return { ...state, error: chunk.message };
  }
}
