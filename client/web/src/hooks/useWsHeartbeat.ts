// Hook que mantiene un WebSocket de control contra /v1/commands del daemon.
// Envía `{"type":"ping"}` cada 20s y mide latencia. Si pasan 10s sin
// un `{"type":"pong"}`, marca la conexión como zombie.
// EP-0003-01.

import { useEffect, useRef, useState } from 'react';

export type WsHeartbeatStatus = 'connecting' | 'open' | 'closed';

export interface UseWsHeartbeatOptions {
  /** Path del WS (default '/v1/commands'). */
  path?: string;
  /** Intervalo entre pings en ms (default 20000). */
  pingIntervalMs?: number;
  /** Timeout sin pong antes de marcar zombie en ms (default 10000). */
  pongTimeoutMs?: number;
  /** Backoff inicial de reconnect en ms (default 1000). */
  initialBackoffMs?: number;
  /** Backoff máximo de reconnect en ms (default 30000). */
  maxBackoffMs?: number;
}

export interface UseWsHeartbeatResult {
  status: WsHeartbeatStatus;
  latencyMs: number | null;
  lastPongAt: number | null;
  isZombie: boolean;
}

const DEFAULTS = {
  path: '/v1/commands',
  pingIntervalMs: 20_000,
  pongTimeoutMs: 10_000,
  initialBackoffMs: 1_000,
  maxBackoffMs: 30_000,
};

export function useWsHeartbeat(
  options: UseWsHeartbeatOptions = {},
): UseWsHeartbeatResult {
  const opts = { ...DEFAULTS, ...options };

  const [status, setStatus] = useState<WsHeartbeatStatus>('connecting');
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [lastPongAt, setLastPongAt] = useState<number | null>(null);
  const [isZombie, setIsZombie] = useState(false);

  // Refs para estado mutable que no debe triggerear re-renders.
  const sentPingAtRef = useRef<number | null>(null);
  const pongTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const backoffRef = useRef<number>(opts.initialBackoffMs);
  const socketRef = useRef<WebSocket | null>(null);
  const cancelledRef = useRef(false);
  // Guard para que el pong timer no se desarme entre que llega el pong y
  // se programa el próximo ping (carrera con re-renders).
  const armedRef = useRef(false);

  useEffect(() => {
    cancelledRef.current = false;
    let activeSocket: WebSocket | null = null;

    function clearPingTimer() {
      if (pingTimerRef.current !== null) {
        clearInterval(pingTimerRef.current);
        pingTimerRef.current = null;
      }
    }

    function clearPongTimer() {
      if (pongTimerRef.current !== null) {
        clearTimeout(pongTimerRef.current);
        pongTimerRef.current = null;
      }
    }

    function sendPing(socket: WebSocket) {
      // WebSocket.OPEN = 1 en el spec; lo referenciamos vía la constante
      // para que el chequeo sea robusto aunque en tests el mock no
      // exponga la constante (la exponemos en MockWebSocket.OPEN).
      if (socket.readyState !== WebSocket.OPEN) return;
      const sentAt = Date.now();
      sentPingAtRef.current = sentAt;
      socket.send(JSON.stringify({ type: 'ping' }));
      // Arma el watchdog de pong.
      clearPongTimer();
      armedRef.current = true;
      pongTimerRef.current = setTimeout(() => {
        // No llegó pong a tiempo → zombie.
        if (!cancelledRef.current) {
          setIsZombie(true);
        }
      }, opts.pongTimeoutMs);
    }

    function connect() {
      if (cancelledRef.current) return;
      if (typeof WebSocket === 'undefined') return;

      const apiBase = (import.meta.env?.VITE_API_BASE as string | undefined) ?? '';
      const wsUrl = opts.path.startsWith('ws')
        ? opts.path
        : apiBase
          ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${apiBase.replace(/^https?:\/\//, '')}${opts.path}`
          : `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}${opts.path}`;

      const socket = new WebSocket(wsUrl);
      activeSocket = socket;
      socketRef.current = socket;
      setStatus('connecting');
      setIsZombie(false);

      socket.addEventListener('open', () => {
        if (cancelledRef.current) return;
        setStatus('open');
        backoffRef.current = opts.initialBackoffMs;
        // Primer ping inmediato al abrir.
        sendPing(socket);
        // Programar pings periódicos.
        clearPingTimer();
        pingTimerRef.current = setInterval(() => sendPing(socket), opts.pingIntervalMs);
      });

      socket.addEventListener('message', (ev: MessageEvent<string>) => {
        if (cancelledRef.current) return;
        let parsed: { type?: string } | null = null;
        try {
          parsed = JSON.parse(ev.data) as { type?: string };
        } catch {
          return; // no es JSON válido, ignorar
        }
        if (parsed?.type !== 'pong') return;

        // Llegó pong. Si esperábamos uno, calculamos latencia.
        clearPongTimer();
        armedRef.current = false;
        setIsZombie(false);
        const now = Date.now();
        setLastPongAt(now);
        if (sentPingAtRef.current !== null) {
          setLatencyMs(now - sentPingAtRef.current);
          sentPingAtRef.current = null;
        }
      });

      socket.addEventListener('error', () => {
        if (cancelledRef.current) return;
        // Forzar cierre para que dispare el reconnect.
        socket.close();
      });

      socket.addEventListener('close', () => {
        if (cancelledRef.current) return;
        clearPingTimer();
        clearPongTimer();
        setStatus('closed');
        socketRef.current = null;
        // Backoff exponencial.
        const wait = backoffRef.current;
        backoffRef.current = Math.min(wait * 2, opts.maxBackoffMs);
        retryTimerRef.current = setTimeout(connect, wait);
      });
    }

    connect();

    return () => {
      cancelledRef.current = true;
      clearPingTimer();
      clearPongTimer();
      if (retryTimerRef.current !== null) {
        clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      if (activeSocket !== null) {
        // El close handler se va a disparar, pero como `cancelledRef` es true
        // no va a re-programar reconnect. Cerramos directo.
        try {
          activeSocket.close();
        } catch {
          // ignore
        }
      }
    };
    // Las opciones se capturan en el closure inicial. Re-crear la conexión
    // ante cada cambio de opts sería agresivo — Spec 1 asume que el caller
    // pasa opciones estables. Si el caller necesita re-configurar, debe
    // desmontar y remontar el hook.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { status, latencyMs, lastPongAt, isZombie };
}
