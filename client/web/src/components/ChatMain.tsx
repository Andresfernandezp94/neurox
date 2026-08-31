// ChatMain — sección central del ChatPanel.
// Contiene: error banner + transcript (mensajes) + history aside.

import type { RefObject } from "react";
import { PanelToggle } from "../shared/components/PanelToggle";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { ChatHistory } from "./ChatHistory";
import { MessageRow } from "./ChatPanel.MessageRow";
import type { Message } from "../types";

export interface ChatMainProps {
  error: string | null;
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
  error,
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
      {error && <ErrorBanner>Error: {error}</ErrorBanner>}

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
