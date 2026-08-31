// searchTranscript.ts — total/idx utilities for in-chat search.
//
// Walks a Message[] and returns textual positions of every match in
// the order they appear in the transcript (top-to-bottom, left-to-right
// inside each chunk). Used by ChatPanel to compute the total and to
// scroll the active match into view.
//
// The search hits *visible* content: user/assistant content, thinking,
// tool name/args/result, approval tool/args/reason, and every timeline
// entry's text/tool/result/reason. Metadata like `metrics`, `ts`, or
// `id` is ignored.

import { countMatches } from "./highlight";
import type { Message, ToolActivity, ApprovalActivity, TimelineEntry } from "../../types";

/** Flat representation of a single searchable text span. */
export interface SearchHit {
  /** `matchIndex` is the global 0-based index across the whole transcript. */
  matchIndex: number;
  /** Local identifier for the span (used by the renderer to know which
   *  field this hit belongs to when wrapping the highlight). */
  spanId: string;
  text: string;
}

function toolText(tool: ToolActivity): string {
  const parts: string[] = [tool.tool];
  if (tool.args != null) parts.push(safeStringify(tool.args));
  if (tool.result) parts.push(tool.result);
  return parts.join("\n");
}

function approvalText(a: ApprovalActivity): string {
  const parts: string[] = [a.tool];
  if (a.args != null) parts.push(safeStringify(a.args));
  if (a.reason) parts.push(a.reason);
  return parts.join("\n");
}

function timelineText(entry: TimelineEntry): string {
  switch (entry.type) {
    case "thinking":
    case "content":
      return entry.text;
    case "tool":
      return [entry.tool, safeStringify(entry.args), entry.result ?? ""].filter(Boolean).join("\n");
    case "approval":
      return [entry.tool, safeStringify(entry.args), entry.reason ?? ""].filter(Boolean).join("\n");
  }
}

function safeStringify(v: unknown): string {
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}

/** How many (case-insensitive) matches of `query` are inside the
 *  transcript's visible content. Used to render the `n/m` counter. */
export function countTranscriptMatches(messages: Message[], query: string): number {
  if (!query || !query.trim()) return 0;
  let total = 0;
  for (const m of messages) {
    if (m.content) total += countMatches(m.content, query);
    if (m.thinking) total += countMatches(m.thinking, query);
    if (m.toolLog) {
      for (const t of m.toolLog) total += countMatches(toolText(t), query);
    }
    if (m.approvals) {
      for (const a of m.approvals) total += countMatches(approvalText(a), query);
    }
    if (m.timeline) {
      for (const e of m.timeline) total += countMatches(timelineText(e), query);
    }
  }
  return total;
}
