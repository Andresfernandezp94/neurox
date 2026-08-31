// Tests del hook useWsHeartbeat. EP-0003-01.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWsHeartbeat } from './useWsHeartbeat';

type Listener = (ev: Event | MessageEvent | CloseEvent) => void;

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  url: string;
  readyState: number = 0; // CONNECTING
  onopen: Listener | null = null;
  onmessage: Listener | null = null;
  onclose: Listener | null = null;
  onerror: Listener | null = null;
  listeners: Record<string, Listener[]> = {};
  sentMessages: string[] = [];

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  addEventListener(type: string, listener: Listener) {
    this.listeners[type] = this.listeners[type] ?? [];
    this.listeners[type].push(listener);
  }

  removeEventListener() {
    // noop for tests
  }

  close() {
    this.readyState = 3; // CLOSED
    this.listeners.close?.forEach((cb) => cb(new CloseEvent('close')));
  }

  send(data: string) {
    this.sentMessages.push(data);
  }

  // helpers for tests
  simulateOpen() {
    this.readyState = 1;
    this.listeners.open?.forEach((cb) => cb(new Event('open')));
  }
  simulateMessage(data: string) {
    this.listeners.message?.forEach((cb) =>
      cb(new MessageEvent('message', { data })),
    );
  }
  simulateError() {
    this.listeners.error?.forEach((cb) => cb(new Event('error')));
  }
}

describe('useWsHeartbeat', () => {
  let OriginalWebSocket: typeof WebSocket;

  beforeEach(() => {
    MockWebSocket.instances = [];
    OriginalWebSocket = globalThis.WebSocket;
    (globalThis as unknown as { WebSocket: unknown }).WebSocket = MockWebSocket;
    vi.useFakeTimers();
  });

  afterEach(() => {
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket =
      OriginalWebSocket;
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('starts in connecting state and opens a WebSocket to /v1/commands', () => {
    const { result } = renderHook(() => useWsHeartbeat());
    expect(result.current.status).toBe('connecting');
    expect(result.current.isZombie).toBe(false);
    expect(result.current.latencyMs).toBeNull();
    expect(MockWebSocket.instances).toHaveLength(1);
    expect(MockWebSocket.instances[0]!.url).toContain('/v1/commands');
  });

  it('sends a ping immediately on open and transitions to open', async () => {
    const { result } = renderHook(() => useWsHeartbeat());
    const socket = MockWebSocket.instances[0]!;

    act(() => {
      socket.simulateOpen();
    });

    expect(result.current.status).toBe('open');
    expect(socket.sentMessages).toEqual([JSON.stringify({ type: 'ping' })]);
  });

  it('measures latency when a pong arrives', async () => {
    const { result } = renderHook(() =>
      useWsHeartbeat({ pingIntervalMs: 1000, pongTimeoutMs: 5000 }),
    );
    const socket = MockWebSocket.instances[0]!;

    await act(async () => {
      socket.simulateOpen();
    });
    // Avanzar 42ms entre el ping y el pong.
    vi.advanceTimersByTime(42);

    await act(async () => {
      socket.simulateMessage(JSON.stringify({ type: 'pong' }));
    });

    expect(result.current.latencyMs).toBe(42);
    expect(result.current.lastPongAt).not.toBeNull();
    expect(result.current.isZombie).toBe(false);
  });

  it('ignores non-pong messages', async () => {
    const { result } = renderHook(() => useWsHeartbeat());
    const socket = MockWebSocket.instances[0]!;

    await act(async () => {
      socket.simulateOpen();
    });
    expect(result.current.latencyMs).toBeNull();

    await act(async () => {
      socket.simulateMessage(JSON.stringify({ type: 'other' }));
    });
    expect(result.current.latencyMs).toBeNull();
    expect(result.current.isZombie).toBe(false);
  });

  it('marks isZombie when pong does not arrive within timeout', async () => {
    const { result } = renderHook(() =>
      useWsHeartbeat({ pongTimeoutMs: 1000 }),
    );
    const socket = MockWebSocket.instances[0]!;

    act(() => {
      socket.simulateOpen();
    });
    expect(result.current.isZombie).toBe(false);

    // Avanzar más allá del pong timeout sin enviar respuesta.
    // Usamos la variante async para que el re-render de React se procese.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1001);
    });
    expect(result.current.isZombie).toBe(true);
  });

  it('clears isZombie when a pong arrives after a previous zombie state', async () => {
    // pongTimeout < pingInterval garantiza que ningún ping periódico
    // re-arme el watchdog antes de que se dispare.
    const { result } = renderHook(() =>
      useWsHeartbeat({ pongTimeoutMs: 500, pingIntervalMs: 5_000 }),
    );
    const socket = MockWebSocket.instances[0]!;

    act(() => {
      socket.simulateOpen();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(501); // expira el primer pong
    });
    expect(result.current.isZombie).toBe(true);

    act(() => {
      socket.simulateMessage(JSON.stringify({ type: 'pong' }));
    });
    expect(result.current.isZombie).toBe(false);
  });

  it('reconnects with backoff after close', async () => {
    const { result } = renderHook(() =>
      useWsHeartbeat({ initialBackoffMs: 500, maxBackoffMs: 4000 }),
    );

    expect(MockWebSocket.instances).toHaveLength(1);
    const first = MockWebSocket.instances[0]!;

    act(() => {
      first.simulateOpen();
    });
    expect(result.current.status).toBe('open');

    act(() => {
      first.simulateError(); // dispara close → reconnect
    });
    expect(result.current.status).toBe('closed');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(MockWebSocket.instances).toHaveLength(2);
  });

  it('doubles backoff on repeated failures (capped at maxBackoffMs)', async () => {
    renderHook(() =>
      useWsHeartbeat({ initialBackoffMs: 100, maxBackoffMs: 800 }),
    );

    const first = MockWebSocket.instances[0]!;
    act(() => {
      first.simulateError();
    });
    // 1er backoff: 100ms
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(MockWebSocket.instances).toHaveLength(2);
    const second = MockWebSocket.instances[1]!;
    act(() => {
      second.simulateError();
    });
    // 2do backoff: 200ms
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(MockWebSocket.instances).toHaveLength(3);
    const third = MockWebSocket.instances[2]!;
    act(() => {
      third.simulateError();
    });
    // 3er backoff: 400ms
    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
    });
    expect(MockWebSocket.instances).toHaveLength(4);
    const fourth = MockWebSocket.instances[3]!;
    act(() => {
      fourth.simulateError();
    });
    // 4to backoff sería 800ms (capped) — esperamos 800ms.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(799);
    });
    expect(MockWebSocket.instances).toHaveLength(4); // todavía no re-conectó
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(MockWebSocket.instances).toHaveLength(5);
  });

  it('cancels ping interval and closes the socket on unmount', () => {
    const { unmount } = renderHook(() => useWsHeartbeat());
    const socket = MockWebSocket.instances[0]!;
    const closeSpy = vi.spyOn(socket, 'close');
    unmount();
    expect(closeSpy).toHaveBeenCalled();
  });

  it('stops sending pings when the socket is closed (RNF-01)', async () => {
    const { result } = renderHook(() =>
      useWsHeartbeat({ pingIntervalMs: 100, initialBackoffMs: 10_000 }),
    );
    const socket = MockWebSocket.instances[0]!;

    act(() => {
      socket.simulateOpen();
    });
    // Recibimos el primer ping.
    expect(socket.sentMessages).toHaveLength(1);

    // Cerrar el socket.
    act(() => {
      socket.simulateError();
    });
    expect(result.current.status).toBe('closed');

    // El reconnect tiene backoff 10000ms — no debe re-conectar en 5s.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    // El setInterval del ping se canceló en el close handler.
    // No hay nuevos pings al socket cerrado.
    expect(socket.readyState).toBe(3);
    // Y no se creó un nuevo socket.
    expect(MockWebSocket.instances).toHaveLength(1);
  });
});
