// VoiceCallOverlay — overlay fullscreen para sesiones de voice call largas.
//
// Diseñado para uso hands-free: el usuario abre el overlay, habla, y el
// agente responde vocalmente (TTS) sin tocar el teclado. La UI muestra
// un VU meter grande, los transcripts en vivo y un botón de colgar.
//
// Usa `useVoiceCall` con `autoSend=true` — el STT final se envía
// automáticamente al voice MCP vía WS, y el audio TTS entrante se
// reproduce por el hook.
//
// EP-0002.

import { useCallback, useEffect, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useVoiceCall } from "../hooks/useVoiceCall";
import { IconClose, IconVoice } from "../shared/components/Icons";

export interface VoiceCallOverlayProps {
  /** Si está abierto. El padre controla la visibilidad. */
  open: boolean;
  /** session_id del chat activo. */
  sessionId: string | null;
  /** Callback cuando el usuario cierra el overlay (Esc, click fuera, botón). */
  onClose: () => void;
}

const STATE_LABEL: Record<string, string> = {
  idle: "Ready",
  connecting: "Connecting…",
  ready: "Listening",
  listening: "Listening",
  thinking: "Thinking…",
  speaking: "Speaking…",
  error: "Error",
};

export function VoiceCallOverlay({ open, sessionId, onClose }: VoiceCallOverlayProps) {
  const voice = useVoiceCall({
    sessionId,
    autoSend: true,
  });

  // Esc para cerrar.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  // Al cerrar, aseguramos que el hook libere todo.
  const handleClose = useCallback(() => {
    voice.stop();
    onClose();
  }, [voice, onClose]);

  if (!open) return null;
  if (typeof document === "undefined") return null;

  const stateLabel = STATE_LABEL[voice.state] ?? voice.state;
  const meterStyle: CSSProperties = {
    "--voice-level": voice.level.toFixed(3),
  } as CSSProperties;

  const node = (
    <div
      className="voice-call-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Voice call"
      data-testid="voice-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) handleClose();
      }}
    >
      <div className="voice-call-overlay__panel" style={meterStyle}>
        <button
          type="button"
          className="voice-call-overlay__close"
          onClick={handleClose}
          aria-label="End call and close"
          title="End call (Esc)"
          data-testid="voice-overlay-close-x"
        >
          <IconClose />
        </button>

        <header className="voice-call-overlay__header">
          <IconVoice />
          <h2>Voice call</h2>
          <span className="voice-call-overlay__state" data-state={voice.state}>
            {stateLabel}
          </span>
        </header>

        <div
          className={`voice-call-overlay__orb voice-call-overlay__orb--${voice.state}`}
          aria-hidden="true"
        >
          <div className="voice-call-overlay__orb-ring" />
          <div className="voice-call-overlay__orb-ring voice-call-overlay__orb-ring--2" />
          <div className="voice-call-overlay__orb-ring voice-call-overlay__orb-ring--3" />
          <div className="voice-call-overlay__orb-core">
            <IconVoice />
          </div>
        </div>

        {/* VU meter — altura proporcional al nivel del mic */}
        <div
          className="voice-call-overlay__vu"
          aria-label="Microphone level"
          role="meter"
          aria-valuemin={0}
          aria-valuemax={1}
          aria-valuenow={Math.round(voice.level * 100)}
        >
          <div
            className="voice-call-overlay__vu-fill"
            style={{ transform: `scaleX(${voice.level})` }}
          />
        </div>

        {/* Transcripts (interim + assistant) */}
        <div className="voice-call-overlay__transcripts" data-testid="voice-overlay-transcripts">
          {voice.partialTranscript && (
            <p className="voice-call-overlay__transcript voice-call-overlay__transcript--interim">
              <span className="voice-call-overlay__transcript-label">You</span>
              <span>{voice.partialTranscript}</span>
            </p>
          )}
          {!voice.partialTranscript && !voice.lastError && (
            <p className="voice-call-overlay__hint">
              {voice.state === "ready" || voice.state === "listening"
                ? "Habla cuando quieras — te escucho."
                : voice.state === "speaking"
                ? "El agente está hablando…"
                : voice.state === "thinking"
                ? "Procesando tu mensaje…"
                : voice.state === "connecting"
                ? "Conectando con el servidor de voz…"
                : "Pulsa Colgar para terminar."}
            </p>
          )}
          {voice.lastError && (
            <p className="voice-call-overlay__error" role="alert">
              {voice.lastError}
            </p>
          )}
        </div>

        <footer className="voice-call-overlay__footer">
          <button
            type="button"
            className="voice-call-overlay__end"
            onClick={handleClose}
            data-testid="voice-overlay-end"
          >
            <IconClose /> Colgar
          </button>
        </footer>
      </div>
    </div>
  );

  return createPortal(node, document.body);
}
