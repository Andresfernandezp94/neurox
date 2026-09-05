// TimelineRenderer — walks a Message.timeline in order and renders
// each entry inline (thinking → tool → approval → content → …).
//
// EP-2026-08-31: the previous implementation hoisted thinking to the
// top of the timeline (see git blame). That made the chat read
// out-of-order when the agent emitted tool calls interleaved with
// reasoning: "Thinking… [some text] Tool: read_file (above the
// thinking block) Tool result Content reply" — confusing. We now
// render strictly in arrival order so the conversation reads
// top-to-bottom exactly as the agent produced it. This matches
// what the user sees in the sidebar (which never reordered) and
// what the SSE stream actually emits.
//
// EP-2026-08-19: cuando el stream arranca y aún no hay entries,
// mostramos un loader "Thinking…" + 3 dots para que la espera sea
// visible (antes esto lo hacía el legacy `ThinkingSection`).

import { Markdown } from "../../shared/components/Markdown";
import type { TimelineEntry } from "../../types";
import { ThinkingNode } from "./ThinkingNode";
import { ToolNode } from "./ToolNode";
import { ApprovalNode } from "./ApprovalNode";
import { StreamingText } from "./StreamingText";

export function TimelineRenderer({
  timeline,
  isStreaming,
  sessionId,
}: {
  timeline: TimelineEntry[];
  isStreaming: boolean;
  sessionId: string | null;
}) {
  const ordered = timeline;
  return (
    <>
      {isStreaming && ordered.length === 0 && (
        <div
          className="chat__row-thinking-loader"
          data-testid="chat-bubble-thinking-loader"
        >
          Thinking{" "}
          <span
            className="chat__row-thinking-dots"
            aria-label="Thinking"
            role="status"
          >
            <span className="chat__row-thinking-dot" />
            <span className="chat__row-thinking-dot" />
            <span className="chat__row-thinking-dot" />
          </span>
        </div>
      )}
      {ordered.map((entry, idx) => {
        // Keys derive from entry identity so React keeps state
        // (toggle open/closed, scroll position) across updates:
        //   - thinking/content collapse to one entry (reducer
        //     appendToTimeline), so a stable per-kind key is enough
        //   - tool uses (tool, iteration) — the reducer guarantees
        //     unique iteration per call within a stream
        //   - approval already keyed by its daemon-issued id
        // idx is appended to disambiguate duplicate entries of the
        // same kind after the no-reorder change.
        switch (entry.type) {
          case "thinking":
            return entry.text.trim() ? (
              <ThinkingNode
                key={`thinking-${idx}`}
                content={entry.text}
                isStreaming={isStreaming}
              />
            ) : null;
          case "tool":
            return (
              <ToolNode
                key={`tool-${entry.tool}-${entry.iteration}-${idx}`}
                activity={{
                  tool: entry.tool,
                  args: entry.args,
                  iteration: entry.iteration,
                  ...(entry.result !== undefined ? { result: entry.result } : {}),
                }}
                isStreaming={isStreaming}
              />
            );
          case "approval":
            return (
              <ApprovalNode
                key={`approval-${entry.id}-${idx}`}
                approval={{
                  id: entry.id,
                  tool: entry.tool,
                  args: entry.args,
                  reason: entry.reason,
                  ...(entry.decision ? { decision: entry.decision } : {}),
                }}
                sessionId={sessionId}
              />
            );
          case "content":
            if (!entry.text) return null;
            return isStreaming ? (
              <div key={`content-${idx}`} className="chat__row-content">
                <StreamingText text={entry.text} />
              </div>
            ) : (
              <div key={`content-${idx}`} className="chat__row-content">
                <Markdown>{entry.text}</Markdown>
              </div>
            );
          default:
            return null;
        }
      })}
    </>
  );
}
