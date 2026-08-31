// API client para el voice MCP (`mcps/voice/`).
//
// El voice MCP es un proceso aparte que escucha en `127.0.0.1:9998` y expone:
//
//   - REST HTTP en `/voice/*` (start, end, speak, set_*, list_*, status)
//   - WebSocket en `/voice/ws?session_id=<uuid>`
//
// En dev, `vite.config.ts` proxy:
//   - `/voice/ws` → `ws://127.0.0.1:9998` (con `ws: true`)
//   - `/voice/*` → `http://127.0.0.1:9998`
//
// para evitar exponer el puerto 9998 directamente al navegador (mismo patrón
// que ya se usa para `/v1/*` → daemon `:7878`). Por eso las URLs de este
// módulo son siempre relativas: la SPA pega contra el mismo origen y el proxy
// de Vite (o el reverse proxy en producción) se encarga del forward.
//
// EP-0002.

import { apiPost, buildApiUrl } from './client';

// ─── Tipos espejo del `manifest.json` del voice MCP ──────────────────────────

/**
 * MiniMax Spanish voice id. La lista completa (47 voces) está en
 * `mcps/voice/src/schema.rs::voice_tool_schemas()`. Aquí exportamos el
 * subconjunto que la UI necesita para el picker.
 */
export type VoiceId =
  | 'Spanish_SereneWoman'
  | 'Spanish_MaturePartner'
  | 'Spanish_Narrator'
  | 'Spanish_Kind-heartedGirl'
  | 'Spanish_ConfidentWoman'
  | 'Spanish_ThoughtfulMan'
  | 'Spanish_SophisticatedLady'
  | 'Spanish_Comedian'
  | 'Spanish_SensibleManager'
  | 'Spanish_ThoughtfulLady';

export type VoiceModel = 'speech-2.8-hd' | 'speech-2.8-turbo';

export type VoiceEmotion =
  | 'happy'
  | 'sad'
  | 'angry'
  | 'fearful'
  | 'disgusted'
  | 'surprised'
  | 'calm'
  | 'fluent'
  | 'whisper';

export type VoiceAudioFormat = 'mp3' | 'pcm' | 'wav' | 'flac';

/** Opciones para iniciar una voice call (mapean a `voice_call_start`). */
export interface VoiceCallStartOptions {
  voice_id?: VoiceId;
  model?: VoiceModel;
  speed?: number;       // 0.5 .. 2.0
  vol?: number;         // 0.0 .. 10.0
  pitch?: number;       // -12 .. 12
  emotion?: VoiceEmotion;
  format?: VoiceAudioFormat;
  bitrate?: 32_000 | 64_000 | 128_000 | 256_000;
  sample_rate?: 8_000 | 16_000 | 22_050 | 24_000 | 32_000 | 44_100;
  channel?: 1 | 2;
}

/** Respuesta de `voice_call_start` (espejo del body que devuelve `voiced`). */
export interface VoiceCallStartResponse {
  session_id: string;
  voice_id: VoiceId;
  model: VoiceModel;
  format: VoiceAudioFormat;
  sample_rate: number;
  bitrate: number;
  channel: number;
}

/** Respuesta de `voice_call_end`. */
export interface VoiceCallEndResponse {
  ok: true;
  session_id: string;
}

// ─── REST endpoints ──────────────────────────────────────────────────────────

/**
 * Inicia una voice call en el voice MCP.
 *
 * `POST /voice/start` con `session_id` (opcional) + overrides por-call.
 * Devuelve la sesión activa con la config final resuelta.
 *
 * Si el voice MCP está caído, la promesa rechaza con `ApiError` 502/503
 * (el proxy de Vite agrega status sintéticos). El caller decide si
 * mostrar un toast, un banner o reintentar.
 */
export function voiceCallStart(
  sessionId: string,
  opts: VoiceCallStartOptions = {},
): Promise<VoiceCallStartResponse> {
  return apiPost<VoiceCallStartResponse>('/voice/start', {
    session_id: sessionId,
    ...opts,
  });
}

/**
 * Termina una voice call activa.
 *
 * `POST /voice/end` con `{ session_id }`. Idempotente: si la call ya
 * terminó, el MCP responde 200 igual (o un 4xx con mensaje claro).
 */
export function voiceCallEnd(sessionId: string): Promise<VoiceCallEndResponse> {
  return apiPost<VoiceCallEndResponse>('/voice/end', { session_id: sessionId });
}

// ─── WebSocket ───────────────────────────────────────────────────────────────

/**
 * Devuelve la URL del WebSocket del voice MCP para una sesión dada.
 *
 * En dev, Vite proxy redirige `/voice/ws` a `ws://127.0.0.1:9998/voice/ws`,
 * así que la SPA pega contra el mismo origen (sin CORS ni hardcodear
 * puertos). En producción, el reverse proxy frente a la SPA debe hacer
 * el mismo forward (futuro EP en el daemon).
 *
 * Usa `buildApiUrl` para que `VITE_API_BASE` (cuando esté seteado en
 * build-time para producción) prependa el dominio correcto.
 */
export function getVoiceWsUrl(sessionId: string): string {
  const base = buildApiUrl('/voice/ws');
  const sep = base.includes('?') ? '&' : '?';
  return `${base}${sep}session_id=${encodeURIComponent(sessionId)}`;
}

// ─── Constantes para tests / UI ─────────────────────────────────────────────

/**
 * Lista corta de voces que mostramos en el picker por default.
 * El endpoint completo está disponible vía `voice_list_voices` del MCP
 * (no expuesto en este módulo todavía; añadir cuando el Settings panel
 * pida selector de voz).
 */
export const DEFAULT_VOICES: ReadonlyArray<{ id: VoiceId; label: string }> = [
  { id: 'Spanish_SereneWoman', label: 'Serene Woman' },
  { id: 'Spanish_Narrator', label: 'Narrator' },
  { id: 'Spanish_Kind-heartedGirl', label: 'Kind-hearted Girl' },
  { id: 'Spanish_ConfidentWoman', label: 'Confident Woman' },
  { id: 'Spanish_ThoughtfulMan', label: 'Thoughtful Man' },
  { id: 'Spanish_Comedian', label: 'Comedian' },
];

/**
 * Estados que puede devolver el WebSocket del voice MCP en un frame
 * `{ "type": "status", "state": ... }`. Espejo del string literal en
 * `mcps/voice/src/session.rs`.
 */
export type VoiceCallStatus = 'ready' | 'thinking' | 'speaking';

/**
 * Estado del cliente de voice call (state machine de `useVoiceCall`).
 * Más estados que `VoiceCallStatus` porque incluye los nuestros
 * (idle, connecting, listening, error).
 */
export type VoiceClientState =
  | 'idle'           // No hay call activa
  | 'connecting'     // Abriendo WS y pidiendo permiso de mic
  | 'ready'          // WS abierto, esperando que el usuario hable
  | 'listening'      // STT activo, capturando voz
  | 'thinking'       // Transcript enviado al MCP, esperando respuesta
  | 'speaking'       // MCP está mandando audio TTS
  | 'error';         // Algo falló (ver `lastError`)
