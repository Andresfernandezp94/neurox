// MicButton — botón de micrófono inline para el ChatPanel.
//
// Aparece junto al botón Send en el footer del chat. Al hacer click,
// abre (o cierra) la voice call. Visual:
//   - idle:        gris, ícono de mic normal
//   - listening:   verde pulsante, muestra VU meter
//   - error:       rojo con tooltip explicando el error
//
// Usa `useVoiceCall` con `autoSend=false` para modo "dictate": el texto
// transcrito aparece en `partialTranscript` (interim) y, cuando se
// finaliza, se entrega al `onUserTranscript` callback. El caller (típico:
// ChatPanel) decide qué hacer — llenar el textarea o auto-enviar.
//
// EP-0002.

import { useCallback, type CSSProperties } from 'react';
import { useVoiceCall } from "../hooks/useVoiceCall";
import type { VoiceClientState } from "../api/voice";
import { IconMic, IconClose } from "../shared/components/Icons";

export interface MicButtonProps {
  /** session_id del chat activo. Mientras es null, el botón está disabled. */
  sessionId: string | null;
  /**
   * Callback cuando el STT produce un resultado final. El componente no
   * decide qué hacer con el texto — eso queda al caller (típico: setear
   * el valor del textarea o enviar vía streamMessage).
   */
  onUserTranscript?: (text: string) => void;
  /** Etiqueta accesible. Default: "Dictate with voice". */
  label?: string;
}

const labelFor = (state: VoiceClientState, sttSupported: boolean): string => {
  if (!sttSupported) return "Voice input not supported in this browser";
  switch (state) {
    case "idle": return "Dictate with voice";
    case "connecting": return "Connecting microphone…";
    case "ready": return "Listening — click to stop";
    case "listening": return "Listening — click to stop";
    case "thinking": return "Processing — click to stop";
    case "speaking": return "Assistant speaking — click to stop";
    case "error": return "Click to retry";
    default: return "Dictate with voice";
  }
};

export function MicButton({
  sessionId,
  onUserTranscript,
  label,
}: MicButtonProps) {
  const voice = useVoiceCall({
    sessionId,
    autoSend: false,
    onUserTranscript,
  });

  const isActive =
    voice.state === "ready" ||
    voice.state === "listening" ||
    voice.state === "thinking" ||
    voice.state === "speaking";
  const disabled = !sessionId || voice.state === "connecting";
  const sttSupported = voice.sttSupported;

  const handleClick = useCallback(() => {
    voice.toggle();
  }, [voice]);

  // VU meter inline (4 barras detrás del ícono cuando está activo).
  const meterStyle: CSSProperties = {
    "--voice-level": voice.level.toFixed(3),
  } as CSSProperties;

  const classes = [
    "mic-button",
    isActive && "mic-button--active",
    voice.state === "listening" && "mic-button--listening",
    voice.state === "error" && "mic-button--error",
    !sttSupported && "mic-button--unsupported",
  ]
    .filter(Boolean)
    .join(" ");

  const tooltip =
    voice.lastError && voice.state === "error"
      ? `${labelFor(voice.state, sttSupported)} (${voice.lastError})`
      : (label ?? labelFor(voice.state, sttSupported));

  return (
    <button
      type="button"
      className={classes}
      onClick={handleClick}
      disabled={disabled || !sttSupported}
      title={tooltip}
      aria-label={tooltip}
      aria-pressed={isActive}
      data-testid="chat-mic"
      data-state={voice.state}
      style={meterStyle}
    >
      <span className="mic-button__icon" aria-hidden="true">
        {isActive ? <IconClose /> : <IconMic />}
      </span>
      {isActive && (
        <span className="mic-button__meter" aria-hidden="true">
          <span className="mic-button__meter-bar" />
          <span className="mic-button__meter-bar" />
          <span className="mic-button__meter-bar" />
          <span className="mic-button__meter-bar" />
        </span>
      )}
      {/* Texto provisional del STT — se muestra como placeholder del
          textarea en el caller. Lo exponemos también acá para debug. */}
      {voice.partialTranscript && (
        <span className="mic-button__partial" aria-live="polite">
          {voice.partialTranscript}
        </span>
      )}
    </button>
  );
}
