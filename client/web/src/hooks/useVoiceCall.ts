// useVoiceCall — hook que conecta la SPA con el voice MCP.
//
// Responsabilidades (todas en uno, a propósito — es el equivalente en
// el cliente de lo que el voice MCP hace en el servidor):
//
//   1. Abre un WebSocket al MCP (`/voice/ws?session_id=...`) cuando
//      el caller llama a `start()`.
//   2. Pide permiso de micrófono con `getUserMedia({ audio: true })`
//      y monta un `AnalyserNode` para el VU meter.
//   3. Inicia STT con `webkitSpeechRecognition` si está disponible;
//      degrada a error si no.
//   4. Cuando STT produce un resultado final: si `autoSend` está activo
//      envía `{type:"text", text}` al MCP; si no, llama a
//      `onUserTranscript(text)` para que el caller decida.
//   5. Recibe frames binarios del WS, los decodifica con
//      `AudioContext.decodeAudioData` y los reproduce.
//   6. Limpia TODO al `stop()` o al unmount: WS, MediaStream,
//      AudioContext, STT.
//
// EP-0002.

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  getVoiceWsUrl,
  type VoiceCallStatus,
  type VoiceClientState,
} from '../api/voice';

// ─── Tipos del navegador que TS no trae por defecto ──────────────────────────

/** `webkitSpeechRecognition` — la API no está en TS lib.dom todavía. */
interface SpeechRecognitionResultAlt {
  transcript: string;
  confidence: number;
}
interface SpeechRecognitionResult {
  readonly length: number;
  isFinal: boolean;
  readonly [index: number]: SpeechRecognitionResultAlt;
  item(index: number): SpeechRecognitionResultAlt;
}
interface SpeechRecognitionEvent extends Event {
  readonly resultIndex: number;
  readonly results: ArrayLike<SpeechRecognitionResult>;
}
interface SpeechRecognitionErrorEvent extends Event {
  readonly error: string;
  readonly message?: string;
}
interface SpeechRecognitionInstance extends EventTarget {
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  start(): void;
  stop(): void;
  abort(): void;
  onresult: ((this: SpeechRecognitionInstance, ev: SpeechRecognitionEvent) => void) | null;
  onerror: ((this: SpeechRecognitionInstance, ev: SpeechRecognitionErrorEvent) => void) | null;
  onend: ((this: SpeechRecognitionInstance, ev: Event) => void) | null;
}
type SpeechRecognitionCtor = new () => SpeechRecognitionInstance;

declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
  }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/** Tipos de frames JSON que manda el voice MCP. */
type ServerFrame =
  | { type: 'status'; state: VoiceCallStatus }
  | { type: 'transcript'; role: 'user' | 'assistant'; text: string }
  | { type: 'error'; message: string };

function isSpeechRecognitionSupported(): boolean {
  if (typeof window === 'undefined') return false;
  return Boolean(window.SpeechRecognition || window.webkitSpeechRecognition);
}

// ─── Hook ───────────────────────────────────────────────────────────────────

export interface UseVoiceCallOptions {
  /** session_id del chat activo. Mientras es null, `start()` es no-op. */
  sessionId: string | null;
  /** Idioma para el STT (default "es-ES"). */
  lang?: string;
  /**
   * Si true (default), los resultados finales de STT se envían automáticamente
   * al voice MCP vía WS como `{type:"text", text}`. Si false, el caller
   * recibe el texto vía `onUserTranscript` y decide (útil para el modo
   * "dictate" del botón inline, donde se rellena el textarea).
   */
  autoSend?: boolean;
  /** Notificado con cada transcripción final del usuario. */
  onUserTranscript?: (text: string) => void;
  /** Notificado con cada transcripción del asistente que llega del MCP. */
  onAssistantTranscript?: (text: string) => void;
  /**
   * Notificado con cada cambio de estado. Útil para tests y para UI que
   * quiera reaccionar a transiciones específicas.
   */
  onStateChange?: (state: VoiceClientState) => void;
}

export interface UseVoiceCallReturn {
  state: VoiceClientState;
  /** Nivel del mic RMS normalizado a [0, 1]. Útil para el VU meter. */
  level: number;
  /** Texto provisional mientras STT está reconociendo (interim result). */
  partialTranscript: string;
  /** Mensaje de error si `state === 'error'`. */
  lastError: string | null;
  /** Abre WS + mic + STT. Idempotente: si ya está corriendo, no hace nada. */
  start: () => Promise<void>;
  /** Cierra WS + mic + STT. Idempotente. */
  stop: () => void;
  /** `start()` si está idle, `stop()` si está corriendo. */
  toggle: () => void;
  /** `true` si STT está soportado en este navegador. */
  sttSupported: boolean;
}

/**
 * Hook que abre la voice call con el voice MCP.
 *
 * Una sola instancia por sesión de chat. El caller debe llamar a
 * `stop()` antes de cambiar de sesión para evitar races con el
 * WebSocket del MCP.
 *
 * @example
 * ```tsx
 * const voice = useVoiceCall({
 *   sessionId,
 *   autoSend: false,
 *   onUserTranscript: (t) => setInput(t),
 * });
 * <button onClick={voice.toggle}>Mic</button>
 * ```
 */
export function useVoiceCall(opts: UseVoiceCallOptions): UseVoiceCallReturn {
  const {
    sessionId,
    lang = 'es-ES',
    autoSend = true,
    onUserTranscript,
    onAssistantTranscript,
    onStateChange,
  } = opts;

  // State público (re-renderable).
  const [state, setStateRaw] = useState<VoiceClientState>('idle');
  const [level, setLevel] = useState(0);
  const [partialTranscript, setPartialTranscript] = useState('');
  const [lastError, setLastError] = useState<string | null>(null);

  // Refs a recursos cuyo ciclo de vida no debe triggerear re-renders.
  const wsRef = useRef<WebSocket | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const audioCtxRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const analyserRafRef = useRef<number | null>(null);
  const recognitionRef = useRef<SpeechRecognitionInstance | null>(null);
  const stoppedRef = useRef(true);
  const reconnectAttemptRef = useRef(0);

  // Callbacks — refs para que no aparezcan en dependencias y causen
  // re-suscripciones del WS cada vez que el caller pase un callback nuevo.
  const onUserTranscriptRef = useRef(onUserTranscript);
  const onAssistantTranscriptRef = useRef(onAssistantTranscript);
  const onStateChangeRef = useRef(onStateChange);
  const autoSendRef = useRef(autoSend);
  useEffect(() => {
    onUserTranscriptRef.current = onUserTranscript;
  }, [onUserTranscript]);
  useEffect(() => {
    onAssistantTranscriptRef.current = onAssistantTranscript;
  }, [onAssistantTranscript]);
  useEffect(() => {
    onStateChangeRef.current = onStateChange;
  }, [onStateChange]);
  useEffect(() => {
    autoSendRef.current = autoSend;
  }, [autoSend]);

  // Wrapper de setState que también notifica al callback.
  const setState = useCallback((next: VoiceClientState) => {
    setStateRaw(next);
    onStateChangeRef.current?.(next);
  }, []);

  // ── Cleanup helpers ────────────────────────────────────────────────────

  const stopAnalyser = useCallback(() => {
    if (analyserRafRef.current !== null) {
      cancelAnimationFrame(analyserRafRef.current);
      analyserRafRef.current = null;
    }
    analyserRef.current = null;
    setLevel(0);
  }, []);

  const stopRecognition = useCallback(() => {
    const r = recognitionRef.current;
    if (!r) return;
    try {
      r.abort();
    } catch {
      /* ignore */
    }
    recognitionRef.current = null;
  }, []);

  const stopMediaStream = useCallback(() => {
    const s = streamRef.current;
    if (!s) return;
    for (const track of s.getTracks()) track.stop();
    streamRef.current = null;
  }, []);

  const stopAudioContext = useCallback(async () => {
    const ctx = audioCtxRef.current;
    if (!ctx) return;
    try {
      if (ctx.state !== 'closed') await ctx.close();
    } catch {
      /* ignore */
    }
    audioCtxRef.current = null;
  }, []);

  const closeWs = useCallback(() => {
    const ws = wsRef.current;
    if (!ws) return;
    const WS = (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket;
    try {
      if (ws.readyState === WS.OPEN) {
        ws.send(JSON.stringify({ type: 'stop' }));
      }
      ws.close();
    } catch {
      /* ignore */
    }
    wsRef.current = null;
  }, []);

  /** Cierra todo. Idempotente. */
  const stop = useCallback(() => {
    stoppedRef.current = true;
    reconnectAttemptRef.current = 0;
    closeWs();
    stopRecognition();
    stopAnalyser();
    stopMediaStream();
    void stopAudioContext();
    setPartialTranscript('');
    setState('idle');
  }, [closeWs, stopRecognition, stopAnalyser, stopMediaStream, stopAudioContext, setState]);

  // ── Audio playback (frames binarios del WS) ─────────────────────────────

  const playBinaryFrame = useCallback(async (data: ArrayBuffer) => {
    const ctx = audioCtxRef.current;
    if (!ctx) return;
    // El voice MCP manda audio MP3 por default. `decodeAudioData` soporta
    // MP3 en navegadores modernos. Si el formato es PCM/WAV/FLAC también
    // funciona; si no, se ignora silenciosamente.
    try {
      // `decodeAudioData` consume el buffer; clonamos por las dudas.
      const audioBuffer = await ctx.decodeAudioData(data.slice(0));
      const source = ctx.createBufferSource();
      source.buffer = audioBuffer;
      source.connect(ctx.destination);
      source.start();
      // `setState('speaking')` se hace en el handler del frame, no acá.
    } catch (e) {
      // Si no se puede decodificar (formato no soportado por el browser,
      // datos corruptos, etc.), seguimos sin cortar la call.
      // eslint-disable-next-line no-console
      console.warn('voice: failed to decode audio frame', e);
    }
  }, []);

  // ── WS message dispatch ─────────────────────────────────────────────────

  const handleServerFrame = useCallback(
    (frame: ServerFrame) => {
      switch (frame.type) {
        case 'status':
          if (frame.state === 'speaking') setState('speaking');
          else if (frame.state === 'thinking') setState('thinking');
          else setState('ready');
          break;
        case 'transcript':
          if (frame.role === 'assistant') {
            onAssistantTranscriptRef.current?.(frame.text);
          }
          // Los transcripts del 'user' que vienen del MCP son eco de lo
          // que el caller envió por `autoSend`. Los ignoramos (ya los
          // tiene vía `onUserTranscript`).
          break;
        case 'error':
          setLastError(frame.message);
          setState('error');
          break;
      }
    },
    [setState],
  );

  // ── WS lifecycle ────────────────────────────────────────────────────────

  const openWs = useCallback(
    (sid: string): Promise<WebSocket> => {
      return new Promise<WebSocket>((resolve, reject) => {
        const url = getVoiceWsUrl(sid);
        // Usamos globalThis.WebSocket explícitamente para que los tests
        // puedan mockearlo con vi.stubGlobal / defineProperty (Node 22+
        // expone un WebSocket nativo que se cuelga con URLs relativas).
        const WS = (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket;
        const ws = new WS(url);
        wsRef.current = ws;

        ws.onopen = () => {
          reconnectAttemptRef.current = 0;
          resolve(ws);
        };

        ws.onmessage = (ev) => {
          if (typeof ev.data === 'string') {
            try {
              const frame = JSON.parse(ev.data) as ServerFrame;
              handleServerFrame(frame);
            } catch {
              // Frame no es JSON — lo ignoramos.
            }
          } else if (ev.data instanceof ArrayBuffer) {
            void playBinaryFrame(ev.data);
          } else if (ev.data instanceof Blob) {
            // Algunos browsers mandan los binary frames como Blob.
            void ev.data.arrayBuffer().then((buf) => playBinaryFrame(buf));
          }
        };

        ws.onerror = () => {
          // onerror no trae detalles útiles. El cierre lo maneja onclose.
        };

        ws.onclose = (ev) => {
          if (wsRef.current === ws) wsRef.current = null;
          // Si el caller todavía quiere estar activo y el cierre no fue
          // solicitado por nosotros, intentamos reconectar con backoff.
          if (!stoppedRef.current && ev.code !== 1000) {
            const attempt = ++reconnectAttemptRef.current;
            const delay = Math.min(1000 * 2 ** Math.min(attempt, 5), 30_000);
            setTimeout(() => {
              if (!stoppedRef.current && sessionId) {
                openWs(sessionId).catch(() => {
                  /* swallow — will retry on next user action */
                });
              }
            }, delay);
          } else {
            setState('idle');
          }
        };

        // Si el caller aborta antes del open, no quedamos colgados.
        const abortTimer = setTimeout(() => {
          if (ws.readyState !== WS.OPEN) {
            try { ws.close(); } catch { /* ignore */ }
            reject(new Error('voice: WS open timed out'));
          }
        }, 10_000);
        const cleanup = () => clearTimeout(abortTimer);
        ws.addEventListener('open', cleanup, { once: true });
        ws.addEventListener('close', cleanup, { once: true });
      });
    },
    [handleServerFrame, playBinaryFrame, sessionId, setState],
  );

  // ── Mic capture + VU meter ──────────────────────────────────────────────

  const startMedia = useCallback(async (): Promise<void> => {
    if (typeof navigator === 'undefined' || !navigator.mediaDevices) {
      throw new Error('voice: navigator.mediaDevices no está disponible');
    }
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    streamRef.current = stream;

    const AudioCtor: typeof AudioContext | undefined =
      window.AudioContext ||
      // Safari viejo expone webkitAudioContext en window
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtor) {
      throw new Error('voice: Web Audio API no soportada');
    }
    const ctx = new AudioCtor();
    audioCtxRef.current = ctx;
    if (ctx.state === 'suspended') {
      await ctx.resume(); // requiere gesto del usuario en algunos browsers
    }

    const source = ctx.createMediaStreamSource(stream);
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 1024;
    source.connect(analyser);
    analyserRef.current = analyser;

    const buf = new Float32Array(analyser.fftSize);
    const tick = () => {
      const a = analyserRef.current;
      if (!a) return;
      a.getFloatTimeDomainData(buf);
      // RMS normalizado a [0, 1].
      let sum = 0;
      for (let i = 0; i < buf.length; i++) sum += buf[i]! * buf[i]!;
      const rms = Math.sqrt(sum / buf.length);
      const normalized = Math.min(1, rms * 4); // amplificar un poco para UI
      setLevel(normalized);
      analyserRafRef.current = requestAnimationFrame(tick);
    };
    analyserRafRef.current = requestAnimationFrame(tick);
  }, []);

  // ── STT ─────────────────────────────────────────────────────────────────

  const startRecognition = useCallback((): boolean => {
    if (!isSpeechRecognitionSupported()) return false;
    const Ctor = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!Ctor) return false;

    const r = new Ctor();
    r.continuous = true;
    r.interimResults = true;
    r.lang = lang;

    r.onresult = (ev) => {
      let interim = '';
      let final = '';
      for (let i = ev.resultIndex; i < ev.results.length; i++) {
        const result = ev.results[i]!;
        const alt = result[0];
        if (!alt) continue;
        if (result.isFinal) final += alt.transcript;
        else interim += alt.transcript;
      }
      if (interim) setPartialTranscript(interim);
      if (final) {
        const text = final.trim();
        setPartialTranscript('');
        if (text.length === 0) return;
        onUserTranscriptRef.current?.(text);
        if (autoSendRef.current) {
          const ws = wsRef.current;
          if (ws && ws.readyState === WebSocket.OPEN) {
            try {
              ws.send(JSON.stringify({ type: 'text', text }));
              setState('thinking');
            } catch (e) {
              // eslint-disable-next-line no-console
              console.warn('voice: failed to send text to MCP', e);
            }
          }
        }
      }
    };

    r.onerror = (ev) => {
      // 'no-speech' y 'aborted' son normales y no deben tirar la call.
      if (ev.error === 'no-speech' || ev.error === 'aborted') return;
      setLastError(`STT error: ${ev.error}`);
      setState('error');
    };

    r.onend = () => {
      // Si el usuario no nos detuvo, reiniciamos (modo continuo).
      if (!stoppedRef.current && recognitionRef.current === r) {
        try { r.start(); } catch { /* ignore — puede ya estar activo */ }
      }
    };

    try {
      r.start();
      recognitionRef.current = r;
      return true;
    } catch {
      return false;
    }
  }, [lang, setState]);

  // ── Public API ──────────────────────────────────────────────────────────

  const start = useCallback(async (): Promise<void> => {
    if (!sessionId) {
      setLastError('voice: no hay session_id activo');
      setState('error');
      return;
    }
    if (!stoppedRef.current) return; // ya corriendo
    stoppedRef.current = false;
    setLastError(null);
    setState('connecting');

    try {
      // 1. Pedir permiso de mic + iniciar AudioContext (debe pasar
      //    dentro del gesto del usuario; `start()` se llama desde un
      //    onClick así que está OK).
      await startMedia();

      // 2. Abrir WS al MCP.
      await openWs(sessionId);

      // 3. Iniciar STT (si está soportado).
      const sttOk = startRecognition();
      if (!sttOk) {
        setLastError('Speech recognition no soportado en este navegador');
        setState('error');
        // No detenemos la call: el caller puede seguir usando el flujo
        // de texto si quiere.
        return;
      }

      setState('ready');
    } catch (e) {
      setLastError((e as Error).message || 'voice: error iniciando');
      setState('error');
      // Cleanup parcial.
      closeWs();
      stopMediaStream();
      void stopAudioContext();
    }
  }, [sessionId, startMedia, openWs, startRecognition, setState, closeWs, stopMediaStream, stopAudioContext]);

  const toggle = useCallback(() => {
    if (stoppedRef.current) void start();
    else stop();
  }, [start, stop]);

  // Cleanup al unmount.
  useEffect(() => {
    return () => {
      stoppedRef.current = true;
      closeWs();
      stopRecognition();
      stopAnalyser();
      stopMediaStream();
      void stopAudioContext();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    state,
    level,
    partialTranscript,
    lastError,
    start,
    stop,
    toggle,
    sttSupported: isSpeechRecognitionSupported(),
  };
}
