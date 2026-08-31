// TimelineRenderer — walks a Message.timeline in order and renders
// each entry inline (thinking → tool → approval → content → …).
//
// EP-2026-08-19: thinking se reordena al inicio del timeline. El wire
// format del backend a veces entrega `content` antes que `thinking`
// (raro pero pasa — ej. agentes que primero escriben la respuesta y
// luego emiten el razonamiento). Para la lectura natural, el toggle
// de Thinking debe quedar ARRIBA de la respuesta del agente. El resto
// del timeline mantiene el orden de llegada.
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

function reorderTimeline(timeline: TimelineEntry[]): TimelineEntry[] {
  const thinking = timeline.filter((e) => e.type === "thinking");
  const others = timeline.filter((e) => e.type !== "thinking");
  return [...thinking, ...others];
}

export function TimelineRenderer({
  timeline,
  isStreaming,
  sessionId,
}: {
  timeline: TimelineEntry[];
  isStreaming: boolean;
  sessionId: string | null;
}) {
  const ordered = reorderTimeline(timeline);
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
      {ordered.map((entry) => {
        // EP-2026-08-19: previously keyed by array index, which
        // shifted when `reorderTimeline()` hoisted new thinking
        // entries to the front — React would then unmount and
        // remount every tool/approval node, losing internal state
        // (toggle open/closed, scroll position). Keys now derive
        // from entry identity:
        //   - thinking/content collapse to one entry (reducer
        //     appendToTimeline), so a stable per-kind key is enough
        //   - tool uses (tool, iteration) — the reducer guarantees
        //     unique iteration per call within a stream
        //   - approval already keyed by its daemon-issued id
        switch (entry.type) {
          case "thinking":
            return entry.text.trim() ? (
              <ThinkingNode
                key="thinking"
                content={entry.text}
                isStreaming={isStreaming}
              />
            ) : null;
          case "tool":
            return (
              <ToolNode
                key={`tool-${entry.tool}-${entry.iteration}`}
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
                key={entry.id}
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
              <div key="content" className="chat__row-content">
                <StreamingText text={entry.text} />
              </div>
            ) : (
              <div key="content" className="chat__row-content">
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
