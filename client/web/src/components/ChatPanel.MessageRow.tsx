// MessageRow — renderiza una fila del transcript.
//
// Composición:
//   header → TimelineRenderer → metrics
//
// `TimelineRenderer` es la única fuente de verdad para el contenido
// del assistant message: renderiza thinking + tools + approvals +
// streaming text en orden cronológico. Si el stream arranca pero aún
// no hay timeline entries, muestra un loader "Thinking + dots". El
// user message no tiene timeline — se renderiza tal cual.

import { memo, useCallback, useState } from "react";
import { IconCopy, IconCheck } from "../shared/components/Icons";
import { TimelineRenderer } from "./chat/TimelineRenderer";
import { Markdown } from "../shared/components/Markdown";
import type { Message } from "../types";

export interface MessageRowProps {
  message: Message;
  /**
   * True si este mensaje es el que se está streameando actualmente.
   * TimelineRenderer lo usa para:
   *  - mostrar el loader "Thinking + dots" cuando el stream arranca
   *  - pasar `isStreaming` al ThinkingNode (cambia "Thinking…" → "Thought")
   *  - renderizar el caret en el content mientras streamea
   */
  isStreaming?: boolean;
}

export const MessageRow = memo(function MessageRow({
  message: m,
  isStreaming = false,
}: MessageRowProps) {
  const isUser = m.role === "user";
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    if (!m.content) return;
    try {
      await navigator.clipboard.writeText(m.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  }, [m.content]);

  const timeline = m.timeline ?? [];

  return (
    <div className={`chat__row chat__row--${m.role}`} data-testid={`chat-bubble-${m.role}`}>
      <div className="chat__row-header">
        <span className="chat__row-role">{m.role}</span>
        {!isUser && m.content && (
          <button
            type="button"
            className="chat__row-copy"
            onClick={() => void handleCopy()}
            title="Copy"
            aria-label="Copy"
          >
            {copied ? <IconCheck /> : <IconCopy />}
          </button>
        )}
      </div>

      {isUser ? (
        <div className="chat__row-content">
          <Markdown>{m.content}</Markdown>
        </div>
      ) : timeline.length === 0 && m.content ? (
        // Historical messages reloaded from /v1/sessions/:id/messages
        // don't carry `timeline` (the daemon only persists content +
        // thinking in the messages table — see MessageRecord in
        // daemon/core/src/session.rs). Render the saved content as a
        // plain markdown bubble so the conversation doesn't show
        // empty rows after a reload / cold start.
        <div className="chat__row-content">
          <Markdown>{m.content}</Markdown>
        </div>
      ) : (
        <TimelineRenderer
          timeline={timeline}
          isStreaming={isStreaming}
          sessionId={m.session_id}
        />
      )}

      {!isUser && m.metrics && (
        <div className="chat__row-stats" data-testid="chat-bubble-metrics">
          <span>
            {m.metrics.durationMs < 1000
              ? `${m.metrics.durationMs}ms`
              : `${(m.metrics.durationMs / 1000).toFixed(2)}s`}
          </span>
          <span className="chat__metrics-sep">·</span>
          <span>{m.metrics.tokens} tok</span>
          <span className="chat__metrics-sep">·</span>
          <span>{m.metrics.tokensPerSec.toFixed(1)} tok/s</span>
        </div>
      )}
    </div>
  );
});
