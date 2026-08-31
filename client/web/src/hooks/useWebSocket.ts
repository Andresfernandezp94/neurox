// Hook que envuelve openEventStream para uso declarativo en React.
// Mantiene un array de eventos y el estado de conexión. EP-0001-02.
// EP-0023-02 R9: agrega ?token=<bearer> al URL del WS upgrade si hay token
// en sessionStorage (el plugin guied lo lee y lo inyecta como header
// Authorization upstream).

import { useEffect, useState } from 'react';
import { type EventStreamStatus } from '../api/events';
import { getToken } from '../api/client';
import type { DaemonEvent } from '../types';

export interface UseWebSocketResult {
  events: DaemonEvent[];
  status: EventStreamStatus;
  clear: () => void;
  /** Forces the WebSocket to close and re-open. Increments on each call. */
  reconnectKey: number;
  reconnect: () => void;
}

export function useWebSocket(path = '/v1/events'): UseWebSocketResult {
  const [events, setEvents] = useState<DaemonEvent[]>([]);
  const [status, setStatus] = useState<EventStreamStatus>('connecting');
  const [reconnectKey, setReconnectKey] = useState(0);

  useEffect(() => {
    let cancelled = false;

    if (typeof window === 'undefined' || typeof WebSocket === 'undefined') {
      return;
    }

    const apiBase = (import.meta.env?.VITE_API_BASE as string | undefined) ?? '';
    const baseWsUrl = path.startsWith('ws')
      ? path
      : apiBase
        ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${apiBase.replace(/^https?:\/\//, '')}${path}`
        : `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}${path}`;

    // EP-0023-02 R9: append Bearer token as query string param if available.
    // The plugin reads `?token=<bearer>` and forwards it as Authorization
    // header upstream. WebSocket API doesn't allow custom headers in
    // the upgrade request, so query string is the standard pattern.
    const token = getToken();
    const wsUrl = token
      ? `${baseWsUrl}${baseWsUrl.includes('?') ? '&' : '?'}token=${encodeURIComponent(token)}`
      : baseWsUrl;

    const socket = new WebSocket(wsUrl);
    setStatus('connecting');

    socket.addEventListener('open', () => {
      if (!cancelled) setStatus('open');
    });
    socket.addEventListener('message', (ev: MessageEvent<string>) => {
      if (cancelled) return;
      try {
        const parsed = JSON.parse(ev.data) as DaemonEvent;
        setEvents((prev) => [...prev, parsed]);
      } catch {
        // ignore non-JSON
      }
    });
    socket.addEventListener('close', () => {
      if (!cancelled) setStatus('closed');
    });
    socket.addEventListener('error', () => socket.close());

    return () => {
      cancelled = true;
      socket.close();
    };
  }, [path, reconnectKey]);

  const clear = () => setEvents([]);
  const reconnect = () => {
    // Re-open the WebSocket without clearing accumulated events —
    // logs are kept in memory so the search bar can still find them.
    setReconnectKey((k) => k + 1);
    setStatus('connecting');
  };

  return { events, status, clear, reconnectKey, reconnect };
}