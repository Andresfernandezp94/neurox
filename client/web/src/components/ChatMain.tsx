// ChatMain — sección central del ChatPanel.
// Contiene: transcript (mensajes) + history aside.
//
// Los errores ya no viven acá: van al stack de notificaciones del shell
// (`NotificationStack`), que es superpuesto y los muestra sobre cualquier
// vista. Esta pieza no sabe de errores.

import type { RefObject } from "react";
import { PanelToggle } from "../shared/components/PanelToggle";
import { ChatHistory } from "./ChatHistory";
import { MessageRow } from "./ChatPanel.MessageRow";
import type { Message } from "../types";

export interface ChatMainProps {
  messages: Message[];
  isStreaming: boolean;
  /**
   * Id del assistant message actualmente streameando (si alguno).
   * ChatMain lo usa para pasar `isStreaming` per-message a MessageRow
   * sin que ChatPanel tenga que saber qué mensajes hay en el transcript.
   */
  streamingMessageId: number | null;
  emptyAgent: string;
  sessionId: string | null;
  scrollRef: RefObject<HTMLDivElement | null>;
  showHistory: boolean;
  onLoadSession: (id: string, summary: string | null) => Promise<void> | void;
  onSearchActiveChange: (i: number) => void;
  searchQuery: string;
}

export function ChatMain({
  messages,
  isStreaming,
  streamingMessageId,
  emptyAgent,
  sessionId,
  scrollRef,
  showHistory,
  onLoadSession,
}: ChatMainProps) {
  return (
    <div className="chat">
      <div
        className="chat__transcript"
        data-testid="chat-transcript"
        ref={scrollRef}
      >
        {messages.length === 0 && !isStreaming && (
          <div className="chat__empty">
            <div className="empty-title">No messages yet</div>
            <div className="muted text-sm">
              Send a message to start chatting with {emptyAgent}.
            </div>
          </div>
        )}
        {messages.map((m) => (
          <MessageRow
            key={m.id}
            message={m}
            isStreaming={m.id === streamingMessageId}
          />
        ))}
      </div>

      <PanelToggle id="chat-history">
        {showHistory && (
          <ChatHistory
            currentSessionId={sessionId}
            onLoadSession={onLoadSession}
            animClass="enter"
          />
        )}
      </PanelToggle>
    </div>
  );
}
