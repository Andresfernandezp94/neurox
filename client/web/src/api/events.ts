// Cliente WebSocket para /v1/events. Maneja auto-reconnect con backoff
// exponencial y parsing de mensajes JSON. EP-0001-02.

import type { DaemonEvent } from '../types';

export type EventStreamStatus = 'connecting' | 'open' | 'closed';

export interface EventStreamHandle {
  events: DaemonEvent[];
  status: EventStreamStatus;
  clear: () => void;
}

interface EventStreamOptions {
  /** Path del WebSocket (default '/v1/events'). */
  path?: string;
  /** Backoff inicial en ms (default 1000). */
  initialBackoffMs?: number;
  /** Backoff máximo en ms (default 30000). */
  maxBackoffMs?: number;
  /** Si se debe reconectar automáticamente (default true). */
  reconnect?: boolean;
}

/**
 * Suscribe al WebSocket /v1/events del daemon.
 *
 * Devuelve un `EventStreamHandle` con la lista de eventos recibidos,
 * el estado de la conexión y un método para limpiarla.
 *
 * En SSR o sin `window`, retorna un handle inerte (sin conexión).
 */
export function openEventStream(opts: EventStreamOptions = {}): EventStreamHandle {
  const path = opts.path ?? '/v1/events';
  const initialBackoffMs = opts.initialBackoffMs ?? 1000;
  const maxBackoffMs = opts.maxBackoffMs ?? 30_000;
  const reconnectEnabled = opts.reconnect ?? true;

  // Estado accesible para el hook. En SSR o sin WebSocket global,
  // retornamos un handle inerte para no romper tests o entornos no-browser.
  if (typeof window === 'undefined' || typeof WebSocket === 'undefined') {
    return { events: [], status: 'closed', clear: () => {} };
  }

  // Resolver URL absoluta (vite proxy mantiene path relativo en dev;
  // en prod con VITE_API_BASE hay que apuntar al backend correcto).
  const apiBase = (import.meta.env?.VITE_API_BASE as string | undefined) ?? '';
  const wsUrl = path.startsWith('ws')
    ? path
    : apiBase
      ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${apiBase.replace(/^https?:\/\//, '')}${path}`
      : `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}${path}`;

  let socket: WebSocket | null = null;
  let backoff = initialBackoffMs;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  let manuallyClosed = false;

  const listeners = {
    status: new Set<(s: EventStreamStatus) => void>(),
    events: new Set<(e: DaemonEvent) => void>(),
  };

  function notifyStatus(status: EventStreamStatus) {
    listeners.status.forEach((cb) => cb(status));
  }
  function notifyEvent(event: DaemonEvent) {
    listeners.events.forEach((cb) => cb(event));
  }

  function connect() {
    socket = new WebSocket(wsUrl);
    notifyStatus('connecting');

    socket.addEventListener('open', () => {
      backoff = initialBackoffMs;
      notifyStatus('open');
    });

    socket.addEventListener('message', (ev: MessageEvent<string>) => {
      try {
        const parsed = JSON.parse(ev.data) as DaemonEvent;
        notifyEvent(parsed);
      } catch {
        // Ignorar mensajes no JSON
      }
    });

    socket.addEventListener('close', () => {
      notifyStatus('closed');
      socket = null;
      if (!manuallyClosed && reconnectEnabled) {
        retryTimer = setTimeout(connect, backoff);
        backoff = Math.min(backoff * 2, maxBackoffMs);
      }
    });

    socket.addEventListener('error', () => {
      // Forzar cierre para activar el reconnect.
      socket?.close();
    });
  }

  connect();

  function clear() {
    manuallyClosed = true;
    if (retryTimer) clearTimeout(retryTimer);
    socket?.close();
    listeners.status.clear();
    listeners.events.clear();
  }

  return { events: [], status: 'connecting', clear };
}

// Helper para tests / consumo externo
export function subscribeToEvents(
  opts: EventStreamOptions,
  onStatus: (s: EventStreamStatus) => void,
  onEvent: (e: DaemonEvent) => void,
): () => void {
  if (typeof window === 'undefined' || typeof WebSocket === 'undefined') {
    return () => {};
  }
  const apiBase = (import.meta.env?.VITE_API_BASE as string | undefined) ?? '';
  const wsUrl = opts.path?.startsWith('ws')
    ? opts.path
    : apiBase
      ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${apiBase.replace(/^https?:\/\//, '')}${opts.path ?? '/v1/events'}`
      : `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}${opts.path ?? '/v1/events'}`;

  const socket = new WebSocket(wsUrl);
  socket.addEventListener('open', () => onStatus('open'));
  socket.addEventListener('close', () => onStatus('closed'));
  socket.addEventListener('message', (ev: MessageEvent<string>) => {
    try {
      const parsed = JSON.parse(ev.data) as DaemonEvent;
      onEvent(parsed);
    } catch {
      // ignore non-JSON
    }
  });

  return () => socket.close();
}
