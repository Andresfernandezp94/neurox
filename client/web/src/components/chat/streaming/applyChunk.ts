// applyStreamChunk — apply a parsed WS stream chunk to a `Message`.
//
// Single source of chunks: the `/v1/events` WebSocket in
// `useChatTabs`. Both the sender's device AND every receiving device
// render the assistant message from the SAME WS broadcast, ordered by
// the daemon-assigned `seq` (PARTE A). The local SSE path
// (ChatPanel → useChatStream) is used ONLY to trigger the turn on the
// backend; it no longer applies chunks.
//
// This removes the previous dual-path reconciliation (text deduper +
// `localStreamingSessionIdRef` skip + `disposed` zombie-socket guard),
// which was fragile and duplicated the stream on the receiving device.
//
// Why a thin wrapper around `streamReducer`?
//
//   - The reducer's `lastNonContentEvent` / `lastToolName` fields are
//     transient (not stored on `Message`). They let the reducer
//     detect the wire-format quirk where the backend embeds a tool's
//     stdout in a `content` chunk and redirect that first chunk to
//     the matching pending tool's `result`. Without these fields we
//     can't tell a "real" content chunk from "tool output that came
//     through the wrong channel".
//
//   - By hydrating the reducer state from the visible `Message`
//     fields on every chunk, both paths share the same wire-format
//     quirk handling, the same timeline collapse (`appendToTimeline`),
//     and the same tool_redirect behaviour.

import type { Message } from "../../../types";
import type { StreamChunk } from "./chunk";
import {
  streamReducer,
  type StreamState,
} from "./reducer";
import type { TimelineEntry } from "../../../types";

/** Transient state that lives between chunks of the same assistant
 *  message. The caller holds one of these per in-flight message id. */
export interface StreamAccumulator {
  lastNonContentEvent: "tool" | "approval" | null;
  lastToolName: string | null;
}

export const initAccumulator = (): StreamAccumulator => ({
  lastNonContentEvent: null,
  lastToolName: null,
});

export interface ApplyChunkResult {
  message: Message;
  acc: StreamAccumulator;
  /** Latest error message seen since the stream started (or null).
   *  Mirrors `StreamState.error`. The caller decides whether to
   *  surface it on a banner. */
  error: string | null;
}

/**
 * Apply a single chunk to a Message.
 *
 * The reducer state is hydrated from the Message's visible fields
 * (content / thinking / toolLog / approvals / timeline). The new state
 * is reduced via `streamReducer` and then mirrored back to a fresh
 * `Message` reference (immutable update). The transient accumulator
 * is updated with the reducer's `lastNonContentEvent` /
 * `lastToolName` so the NEXT chunk in the same stream can make the
 * right decision.
 *
 * Safe to call from both the SSE path (raw chunks parsed by
 * `parseStreamChunk` from a `text/event-stream` body) and the WS path
 * (events received via `/v1/events` after the daemon's broadcast
 * forwarder emits them on the global event bus — see
 * `daemon/core/src/router/http.rs::post_message_stream`).
 */
export function applyStreamChunk(
  msg: Message,
  chunk: StreamChunk,
  acc: StreamAccumulator,
): ApplyChunkResult {
  const state: StreamState = {
    content: msg.content ?? "",
    thinking: msg.thinking ?? "",
    toolLog: msg.toolLog ?? [],
    approvals: msg.approvals ?? [],
    timeline: msg.timeline ?? [],
    error: null,
    lastNonContentEvent: acc.lastNonContentEvent,
    lastToolName: acc.lastToolName,
  };
  const next = streamReducer(state, chunk);
  return {
    message: {
      ...msg,
      content: next.content,
      thinking: next.thinking,
      toolLog: next.toolLog,
      approvals: next.approvals,
      timeline: next.timeline,
    },
    acc: {
      lastNonContentEvent: next.lastNonContentEvent,
      lastToolName: next.lastToolName,
    },
    error: next.error,
  };
}

// ─── Final reconciliation with the backend's ground truth ────────────────
//
// EP-2026-09-05: a RECEIVING device (and sometimes the sender) can miss
// the FIRST few stream chunks of a turn — the `/v1/events` WS wasn't
// attached to the session yet, or the tab didn't exist when the opening
// `content` chunks were broadcast. The streamed `timeline` then starts
// mid-sentence (e.g. "de texto:" instead of "¡Claro que sí! … de
// texto:"). The reply is rendered from `timeline`, so the gap is
// visible even though the daemon's final `MessageAppended` carries the
// COMPLETE `content` + `thinking`.
//
// This function rebuilds a canonical timeline from the backend's final
// text, so the rendered reply is always complete:
//
//   - If the streamed timeline has NO tool/approval entries (a
//     plain text reply — the common case), rebuild it as
//     `[thinking?, content?]` straight from the final fields. Any
//     missing opening chunks are recovered.
//
//   - If it DOES have tool/approval entries, we can't safely re-thread
//     text around them, so we keep the streamed timeline but overwrite
//     the LAST `content` entry (and the first `thinking` entry) with
//     the backend's full text. This guarantees the final reply text is
//     complete without reordering tool activity. If there's no content
//     entry yet, we append one.
//
// Pure: returns a new TimelineEntry[]. `content`/`thinking` are the
// backend's final values from `MessageAppended`.
export function reconcileTimelineWithFinal(
  timeline: TimelineEntry[],
  content: string,
  thinking: string | undefined,
): TimelineEntry[] {
  const hasToolOrApproval = timeline.some(
    (e) => e.type === "tool" || e.type === "approval",
  );

  if (!hasToolOrApproval) {
    // Plain text reply — rebuild cleanly from the ground truth.
    const rebuilt: TimelineEntry[] = [];
    if (thinking && thinking.trim()) {
      rebuilt.push({ type: "thinking", text: thinking });
    }
    if (content) {
      rebuilt.push({ type: "content", text: content });
    }
    // If the backend somehow sent neither, keep whatever we streamed
    // (better than blanking the row).
    return rebuilt.length > 0 ? rebuilt : timeline;
  }

  // Tool/approval present: preserve structure, fix the text entries.
  let contentFixed = false;
  const next = timeline
    .slice()
    .reverse()
    .map((e) => {
      if (!contentFixed && e.type === "content") {
        contentFixed = true;
        return { ...e, text: content };
      }
      return e;
    })
    .reverse();
  if (!contentFixed && content) {
    next.push({ type: "content", text: content });
  }
  // Fix the first thinking entry with the complete thinking text.
  if (thinking && thinking.trim()) {
    let thinkingFixed = false;
    for (let i = 0; i < next.length; i++) {
      const e = next[i]!;
      if (e.type === "thinking") {
        next[i] = { ...e, text: thinking };
        thinkingFixed = true;
        break;
      }
    }
    if (!thinkingFixed) {
      next.unshift({ type: "thinking", text: thinking });
    }
  }
  return next;
}
