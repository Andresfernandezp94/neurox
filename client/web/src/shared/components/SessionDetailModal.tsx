// SessionDetailModal — Modal overlay que muestra el contenido de una sesión.
// Cierra con: click en backdrop, tecla Esc, botón ✕.

import { useCallback, useEffect } from "react";
import { SessionDetail } from "./SessionDetail";
import type { Message, SessionSummary } from "../../types";

export interface SessionDetailModalProps {
  sessionId: string | null;
  session?: SessionSummary | null;
  messages: Message[];
  loading?: boolean;
  onClose: () => void;
}

export function SessionDetailModal({
  sessionId,
  session,
  messages,
  loading = false,
  onClose,
}: SessionDetailModalProps) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [onClose],
  );

  useEffect(() => {
    if (!sessionId) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [sessionId, handleKeyDown]);

  if (!sessionId) return null;

  return (
    <div
      className="session-detail-modal"
      role="dialog"
      aria-modal="true"
      aria-label="Session detail"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="session-detail-modal__container">
        <button
          className="session-detail-modal__close"
          onClick={onClose}
          aria-label="Close"
          title="Close (Esc)"
        >
          ✕
        </button>
        <SessionDetail
          sessionId={sessionId}
          session={session}
          messages={messages}
          loading={loading}
        />
      </div>
    </div>
  );
}
