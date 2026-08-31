// SessionDetail — Vista detalle de una sesión con sus mensajes.
// Cuando sessionId es null, muestra un placeholder.

import type { Message, SessionSummary } from "../../types";

export interface SessionDetailProps {
  sessionId: string | null;
  session?: SessionSummary | null;
  messages: Message[];
  loading?: boolean;
}

function MessageBubble({ message }: { message: Message }) {
  const bgByRole: Record<string, string> = {
    user: "var(--bg-elevated)",
    assistant: "var(--accent-soft, rgba(99, 102, 241, 0.08))",
    system: "var(--bg-tertiary)",
    tool: "var(--bg-tertiary)",
  };
  return (
    <div
      className={`session-detail__message session-detail__message--${message.role}`}
      style={{
        background: bgByRole[message.role] ?? "var(--bg-tertiary)",
        fontStyle: message.role === "tool" ? "italic" : "normal",
      }}
    >
      <div className="session-detail__message-meta">
        <span className="session-detail__message-role">{message.role}</span>
        <span className="session-detail__message-ts">{message.ts}</span>
        {message.metrics && (
          <span className="session-detail__message-metrics">
            {message.metrics.tokens}t · {(message.metrics.tokensPerSec ?? 0).toFixed(1)}/s
          </span>
        )}
      </div>
      <div className="session-detail__message-content">{message.content}</div>
    </div>
  );
}

export function SessionDetail({
  sessionId,
  session,
  messages,
  loading = false,
}: SessionDetailProps) {
  if (!sessionId) {
    return (
      <div className="session-detail session-detail--empty card">
        <div className="session-detail__placeholder-icon">💬</div>
        <div className="session-detail__placeholder-title">No session selected</div>
        <div className="session-detail__placeholder-subtitle">
          Pick a session from the list to see its messages here.
        </div>
      </div>
    );
  }

  return (
    <div className="session-detail card">
      <div className="session-detail__header">
        <div className="session-detail__title-block">
          <h3 className="session-detail__title">
            {session?.summary ?? `Session ${sessionId.slice(0, 8)}…`}
          </h3>
          {session && (
            <div className="session-detail__submeta">
              <span className="session-detail__submeta-agent">{session.agent_id}</span>
              <span className="session-detail__submeta-dot">·</span>
              <span className="session-detail__submeta-id" title={sessionId}>
                {sessionId.slice(0, 8)}…
              </span>
            </div>
          )}
        </div>
      </div>

      {loading && <div className="loading">Loading messages…</div>}

      <div className="session-detail__messages">
        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} />
        ))}
        {messages.length === 0 && !loading && (
          <div className="empty">
            <div className="empty-title">No messages.</div>
          </div>
        )}
      </div>
    </div>
  );
}
