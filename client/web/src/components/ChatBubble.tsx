// ChatBubble.tsx — burbuja flotante estilo soporte (Intercom/Drift style).
// Trigger redondo en bottom-right (fuera de .app-main para que flote sobre
// cualquier panel). Al click, abre un mini chat "simple" — sin sessions
// list, sin tabs, sin agent picker — solo el input y los mensajes.
//
// La sesión default se crea de forma lazy al primer mensaje y se
// reutiliza para el resto de la conversación del bubble. Cada envío es un
// POST /v1/sessions/:id/messages al daemon (mismo endpoint que usa el
// ChatPanel completo, sólo que sin streaming).

import { useCallback, useEffect, useRef, useState } from "react";
import { IconChat, IconClose, IconSend } from "../shared/components/Icons";
import { createSession, sendMessage } from "../api/sessions";
import { useDefaultAgentId } from "../hooks/useDefaultAgentId";

interface BubbleMessage {
  id: number;
  role: "user" | "agent";
  content: string;
  ts: number;
}

let nextBubbleId = 1;

export function ChatBubble() {
  // Resolved at runtime from the daemon's `/v1/agents` snapshot. Falls
  // back to `null` when the daemon is unreachable — we surface that as
  // a visible error rather than sending a stale hardcoded id that
  // the daemon would reject with a confusing `agent not found`.
  const defaultAgentId = useDefaultAgentId();
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<BubbleMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, pending]);

  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const ensureSession = useCallback(async (): Promise<string> => {
    if (sessionId) return sessionId;
    const created = await createSession(defaultAgentId ?? "");
    setSessionId(created.session_id);
    return created.session_id;
  }, [sessionId]);

  const send = useCallback(async () => {
    const text = draft.trim();
    if (!text || pending) return;
    setError(null);
    const userMsg: BubbleMessage = {
      id: nextBubbleId++,
      role: "user",
      content: text,
      ts: Date.now(),
    };
    setMessages((prev) => [...prev, userMsg]);
    setDraft("");
    setPending(true);
    try {
      const id = await ensureSession();
      const res = (await sendMessage(
        id,
        defaultAgentId ?? "",
        text,
      )) as {
        session_id: string;
        agent_id: string;
        result?: { text?: string; thinking?: string };
      };
      const reply =
        res.result?.text ??
        res.result?.thinking ??
        "(empty response)";
      const agentMsg: BubbleMessage = {
        id: nextBubbleId++,
        role: "agent",
        content: reply,
        ts: Date.now(),
      };
      setMessages((prev) => [...prev, agentMsg]);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setPending(false);
    }
  }, [draft, pending, ensureSession]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  };

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-label={open ? "Close quick chat" : "Open quick chat"}
        title={open ? "Close quick chat" : "Quick chat"}
        data-testid="chat-bubble-trigger"
        style={{
          position: "fixed",
          right: 24,
          bottom: 24,
          width: 56,
          height: 56,
          borderRadius: "50%",
          border: "none",
          background: open
            ? "var(--bg-elevated)"
            : "var(--accent)",
          color: open ? "var(--text-primary)" : "#1a1a1a",
          display: "grid",
          placeItems: "center",
          cursor: "pointer",
          boxShadow: "0 8px 24px rgba(0, 0, 0, 0.45)",
          zIndex: 200,
          transition:
            "transform 0.2s cubic-bezier(0.4, 0, 0.2, 1), background 0.2s ease",
        }}
      >
        {open ? <IconClose /> : <IconChat />}
      </button>

      {open && (
        <div
          data-testid="chat-bubble-panel"
          role="dialog"
          aria-label="Quick chat"
          style={{
            position: "fixed",
            right: 24,
            bottom: 96,
            width: 360,
            maxWidth: "calc(100vw - 48px)",
            height: 480,
            maxHeight: "calc(100dvh - 132px)",
            display: "flex",
            flexDirection: "column",
            background: "var(--surface-translucent)",
            backdropFilter: "var(--chrome-blur)",
            border: "1px solid var(--border)",
            borderRadius: 14,
            boxShadow: "0 16px 48px rgba(0, 0, 0, 0.5)",
            zIndex: 199,
            overflow: "hidden",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "10px 14px",
              borderBottom: "1px solid var(--border)",
              color: "var(--text-primary)",
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, monospace",
              fontSize: 13,
            }}
          >
            <span style={{ fontWeight: 600 }}>Quick chat</span>
            <span style={{ opacity: 0.6, fontSize: 11 }}>
              {sessionId ? "default · live" : "default"}
            </span>
          </div>

          <div
            ref={listRef}
            data-testid="chat-bubble-messages"
            style={{
              flex: 1,
              minHeight: 0,
              overflowY: "auto",
              padding: "12px 14px",
              display: "flex",
              flexDirection: "column",
              gap: 8,
            }}
          >
            {messages.length === 0 && !pending && (
              <p
                style={{
                  color: "var(--text-muted)",
                  fontSize: 12,
                  textAlign: "center",
                  padding: "20px 8px",
                  margin: "auto 0",
                }}
              >
                Ask anything. Enter to send, Shift+Enter for newline.
              </p>
            )}
            {messages.map((m) => (
              <BubbleMessageBubble key={m.id} message={m} />
            ))}
            {pending && (
              <div
                data-testid="chat-bubble-pending"
                style={{
                  alignSelf: "flex-start",
                  background: "var(--bg-elevated)",
                  color: "var(--text-primary)",
                  padding: "8px 12px",
                  borderRadius: 10,
                  fontSize: 12,
                  opacity: 0.7,
                }}
              >
                <span>thinking…</span>
              </div>
            )}
          </div>

          {error && (
            <div
              data-testid="chat-bubble-error"
              style={{
                margin: "0 14px 8px",
                padding: "6px 10px",
                background: "var(--status-danger-bg)",
                color: "var(--status-danger)",
                borderRadius: 8,
                fontSize: 11,
              }}
            >
              {error}
            </div>
          )}

          <form
            onSubmit={(e) => {
              e.preventDefault();
              void send();
            }}
            style={{
              display: "flex",
              alignItems: "flex-end",
              gap: 8,
              padding: "10px 12px",
              borderTop: "1px solid var(--border)",
              background: "var(--bg)",
            }}
          >
            <textarea
              ref={inputRef}
              data-testid="chat-bubble-input"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder="Type a message..."
              rows={1}
              style={{
                flex: 1,
                resize: "none",
                border: "1px solid var(--border)",
                borderRadius: 8,
                background: "var(--surface)",
                color: "var(--text-primary)",
                padding: "8px 10px",
                fontSize: 13,
                fontFamily: "inherit",
                outline: "none",
                minHeight: 36,
                maxHeight: 120,
                lineHeight: 1.4,
              }}
            />
            <button
              type="submit"
              data-testid="chat-bubble-send"
              disabled={!draft.trim() || pending}
              aria-label="Send"
              title="Send"
              style={{
                width: 36,
                height: 36,
                borderRadius: 8,
                border: "none",
                background: "var(--accent)",
                color: "#1a1a1a",
                display: "grid",
                placeItems: "center",
                cursor: draft.trim() && !pending ? "pointer" : "not-allowed",
                opacity: draft.trim() && !pending ? 1 : 0.5,
                flexShrink: 0,
              }}
            >
              <IconSend />
            </button>
          </form>
        </div>
      )}
    </>
  );
}

function BubbleMessageBubble({ message }: { message: BubbleMessage }) {
  const isUser = message.role === "user";
  return (
    <div
      data-testid={`chat-bubble-msg-${message.role}`}
      style={{
        alignSelf: isUser ? "flex-end" : "flex-start",
        maxWidth: "85%",
        background: isUser
          ? "var(--accent-soft)"
          : "var(--bg-elevated)",
        color: "var(--text-primary)",
        padding: "8px 12px",
        borderRadius: 10,
        fontSize: 12,
        lineHeight: 1.5,
        whiteSpace: "pre-wrap",
        wordBreak: "break-word",
      }}
    >
      {message.content}
    </div>
  );
}
