// ThinkingNode — collapsible thinking block in the chat timeline.
//
// EP-0026-rev: single-button toggle styled like the args/result
// previews. Label flips Thinking… → Thought when the stream ends,
// with a preview of the first line of the reasoning so the user
// can decide whether to expand.
//
// EP-0026-rev-fix: starts collapsed. The user's open/closed choice
// persists across the streaming → post-stream transition (no
// auto-collapse) so the "Thought" label inherits whatever state
// the user left the thinking in.

import { useEffect, useRef, useState } from "react";

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

export function ThinkingNode({ content, isStreaming }: { content: string; isStreaming: boolean }) {
  // EP-0026-rev-fix: starts collapsed. User's choice persists.
  const [open, setOpen] = useState(false);

  // EP-0024-UX: time the thinking.
  const startTimeRef = useRef<number | null>(null);
  const endTimeRef = useRef<number | null>(null);
  const [now, setNow] = useState<number>(() => Date.now());

  useEffect(() => {
    if (content && startTimeRef.current === null) {
      startTimeRef.current = Date.now();
    }
  }, [content]);

  useEffect(() => {
    if (!isStreaming && startTimeRef.current !== null && endTimeRef.current === null) {
      endTimeRef.current = Date.now();
    }
  }, [isStreaming]);

  useEffect(() => {
    if (!isStreaming || startTimeRef.current === null) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [isStreaming]);

  const durationMs =
    startTimeRef.current === null
      ? null
      : (endTimeRef.current ?? now) - startTimeRef.current;
  const durationLabel =
    durationMs === null ? "" : formatDuration(durationMs);

  // EP-0026-rev-fix: preview of the first line of the thinking.
  const thinkingPreview = content.trim()
    ? (() => {
        const firstLine = content.split("\n")[0] ?? "";
        return firstLine.length > 80
          ? firstLine.slice(0, 80) + "…"
          : firstLine;
      })()
    : "";

  // EP-0026-rev-fix: body visible only when user has expanded it.
  // Starts collapsed, choice persists.
  const effectiveOpen = open;

  function handleToggle() {
    setOpen((o) => !o);
  }

  return (
    <div className="chat__timeline-node chat__timeline-node--thinking">
      <div className="chat__timeline-body">
        <button
          type="button"
          className={`chat__timeline-result-preview${effectiveOpen ? " chat__timeline-result-preview--open" : ""}`}
          onClick={handleToggle}
          aria-expanded={effectiveOpen}
          aria-label={effectiveOpen ? "Collapse thinking" : "Expand thinking"}
          data-testid="thinking-toggle"
        >
          <span className="chat__timeline-result-preview-glyph" aria-hidden="true">
            {effectiveOpen ? "−" : "+"}
          </span>
          <span className="chat__timeline-label">
            {isStreaming ? "Thinking…" : "Thought"}
          </span>
          {!isStreaming && thinkingPreview && (
            <span className="chat__timeline-detail" data-testid="thinking-preview">
              {" · "}{thinkingPreview}
            </span>
          )}
          {durationLabel && (
            <span className="chat__timeline-detail" data-testid="thinking-duration">
              {" · "}{durationLabel}
            </span>
          )}
        </button>
        {effectiveOpen && (
          <div className="chat__thinking-body" data-testid="thinking-content">
            <div className="chat__streaming-text chat__thinking-body-inner">
              {content}
              {isStreaming && <span className="chat__caret" aria-hidden="true" />}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
