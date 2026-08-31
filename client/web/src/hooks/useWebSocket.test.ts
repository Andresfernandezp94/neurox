// Tests del hook useWebSocket. EP-0001-02.
// EP-0023-02 R9: tests para ?token=<bearer> query string injection.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWebSocket } from './useWebSocket';
import { setToken } from '../api/client';

type Listener = (ev: Event | MessageEvent | CloseEvent) => void;

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  url: string;
  readyState: number = 0; // CONNECTING
  onopen: Listener | null = null;
  onmessage: Listener | null = null;
  onclose: Listener | null = null;
  onerror: Listener | null = null;
  listeners: Record<string, Listener[]> = {};

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

describe('useWebSocket', () => {
  let OriginalWebSocket: typeof WebSocket;

  beforeEach(() => {
    MockWebSocket.instances = [];
    OriginalWebSocket = globalThis.WebSocket;
    (globalThis as unknown as { WebSocket: unknown }).WebSocket = MockWebSocket;
  });

  afterEach(() => {
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket =
      OriginalWebSocket;
    vi.restoreAllMocks();
  });

  it('starts in connecting state', () => {
    const { result } = renderHook(() => useWebSocket('/v1/events'));
    expect(result.current.status).toBe('connecting');
    expect(result.current.events).toEqual([]);
    expect(MockWebSocket.instances).toHaveLength(1);
  });

  it('transitions to open when the socket opens', async () => {
    const { result } = renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;

    await act(async () => {
      socket.simulateOpen();
    });

    expect(result.current.status).toBe('open');
  });

  it('parses JSON messages into the events array', async () => {
    const { result } = renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;

    await act(async () => {
      socket.simulateOpen();
      socket.simulateMessage(JSON.stringify({ type: 'agent.started', id: 'default' }));
      socket.simulateMessage(JSON.stringify({ type: 'tool.invoked', name: 'shell' }));
    });

    expect(result.current.events).toHaveLength(2);
    expect(result.current.events[0]).toMatchObject({ type: 'agent.started', id: 'default' });
    expect(result.current.events[1]).toMatchObject({ type: 'tool.invoked', name: 'shell' });
  });

  it('ignores non-JSON messages', async () => {
    const { result } = renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;

    await act(async () => {
      socket.simulateOpen();
      socket.simulateMessage('not json');
    });

    expect(result.current.events).toHaveLength(0);
  });

  it('clear() empties the events array', async () => {
    const { result } = renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;

    await act(async () => {
      socket.simulateOpen();
      socket.simulateMessage(JSON.stringify({ type: 'test' }));
    });
    expect(result.current.events).toHaveLength(1);

    act(() => result.current.clear());
    expect(result.current.events).toEqual([]);
  });

  it('closes the socket on unmount', () => {
    const { unmount } = renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;
    const closeSpy = vi.spyOn(socket, 'close');
    unmount();
    expect(closeSpy).toHaveBeenCalled();
  });

  // EP-0023-02 R9: ?token=<bearer> query string for WS auth.

  it('appends ?token=<bearer> when sessionStorage has a token', () => {
    setToken('secret-xyz');
    renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;
    expect(socket.url).toContain('?token=secret-xyz');
  });

  it('does not append ?token when sessionStorage is empty', () => {
    setToken(null);
    renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;
    expect(socket.url).not.toContain('token=');
  });

  it('URL-encodes special characters in token', () => {
    setToken('abc+def=ghi');
    renderHook(() => useWebSocket('/v1/events'));
    const socket = MockWebSocket.instances[0]!;
    // Plus should be %2B, equals should be %3D
    expect(socket.url).toContain('token=abc%2Bdef%3Dghi');
  });

  it('uses & separator if path already has query string', () => {
    setToken('secret');
    renderHook(() => useWebSocket('/v1/events?other=value'));
    const socket = MockWebSocket.instances[0]!;
    expect(socket.url).toContain('?other=value&token=secret');
  });
});
