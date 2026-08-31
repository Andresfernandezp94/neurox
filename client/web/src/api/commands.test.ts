// Tests del CommandsClient. EP-0003-05.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { CommandsClient } from './commands';

type Listener = (ev: Event | MessageEvent | CloseEvent) => void;

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  url: string;
  readyState: number = 0;
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
    // noop
  }

  close() {
    this.readyState = 3;
    this.listeners.close?.forEach((cb) => cb(new CloseEvent('close')));
  }

  send(data: string) {
    if (this.readyState !== MockWebSocket.OPEN) {
      throw new Error('WS not open');
    }
    this.sentMessages.push(data);
  }

  simulateOpen() {
    this.readyState = 1;
    this.listeners.open?.forEach((cb) => cb(new Event('open')));
  }
  simulateMessage(data: string) {
    this.listeners.message?.forEach((cb) =>
      cb(new MessageEvent('message', { data })),
    );
  }
  simulateClose() {
    this.readyState = 3;
    this.listeners.close?.forEach((cb) => cb(new CloseEvent('close')));
  }
  simulateError() {
    this.listeners.error?.forEach((cb) => cb(new Event('error')));
  }
}

describe('CommandsClient', () => {
  let OriginalWebSocket: typeof WebSocket;

  beforeEach(() => {
    MockWebSocket.instances = [];
    OriginalWebSocket = globalThis.WebSocket;
    (globalThis as unknown as { WebSocket: unknown }).WebSocket = MockWebSocket;
  });

  afterEach(() => {
    (globalThis as unknown as { WebSocket: typeof WebSocket }).WebSocket =
      OriginalWebSocket;
    vi.useRealTimers();
  });

  it('init() opens a WebSocket to /v1/commands', () => {
    const c = new CommandsClient();
    c.init();
    expect(MockWebSocket.instances).toHaveLength(1);
    expect(MockWebSocket.instances[0]!.url).toContain('/v1/commands');
    c.close();
  });

  it('init() is idempotent', () => {
    const c = new CommandsClient();
    c.init();
    c.init();
    expect(MockWebSocket.instances).toHaveLength(1);
    c.close();
  });

  it('send() rejects if WS not open', async () => {
    const c = new CommandsClient();
    c.init();
    await expect(c.send({ type: 'list_agents' })).rejects.toThrow(/not open/);
    c.close();
  });

  it('send() resolves when matching response with request_id arrives', async () => {
    const c = new CommandsClient({ commandTimeoutMs: 5000 });
    c.init();
    const socket = MockWebSocket.instances[0]!;
    socket.simulateOpen();
    expect(c.isOpen()).toBe(true);

    const promise = c.send({ type: 'start_agent', id: 'default' });
    expect(socket.sentMessages).toHaveLength(1);
    const sent = JSON.parse(socket.sentMessages[0]!);
    expect(sent.type).toBe('start_agent');
    expect(sent.id).toBe('default');
    expect(sent.request_id).toBeTruthy();

    socket.simulateMessage(
      JSON.stringify({
        type: 'agent_started',
        id: 'default',
        ok: true,
        request_id: sent.request_id,
      }),
    );
    await expect(promise).resolves.toMatchObject({ type: 'agent_started', ok: true });
    c.close();
  });

  it('send() rejects on error response', async () => {
    const c = new CommandsClient({ commandTimeoutMs: 5000 });
    c.init();
    const socket = MockWebSocket.instances[0]!;
    socket.simulateOpen();

    const promise = c.send({ type: 'start_agent', id: 'default' });
    const sent = JSON.parse(socket.sentMessages[0]!);
    socket.simulateMessage(
      JSON.stringify({ type: 'error', message: 'agent not found', request_id: sent.request_id }),
    );
    await expect(promise).rejects.toThrow(/agent not found/);
    c.close();
  });

  it('send() times out if no response arrives', async () => {
    vi.useFakeTimers();
    const c = new CommandsClient({ commandTimeoutMs: 100 });
    c.init();
    const socket = MockWebSocket.instances[0]!;
    socket.simulateOpen();

    const promise = c.send({ type: 'start_agent', id: 'default' });
    // Advance past timeout.
    await vi.advanceTimersByTimeAsync(101);
    await expect(promise).rejects.toThrow(/timeout/);
    c.close();
    vi.useRealTimers();
  });

  it('records latency after successful round-trip', async () => {
    const c = new CommandsClient({ commandTimeoutMs: 5000 });
    c.init();
    const socket = MockWebSocket.instances[0]!;
    socket.simulateOpen();
    expect(c.getLatencyMs()).toBeNull();

    const promise = c.send({ type: 'list_agents' });
    const sent = JSON.parse(socket.sentMessages[0]!);
    // Simular 20ms de latencia.
    await new Promise((r) => setTimeout(r, 20));
    socket.simulateMessage(
      JSON.stringify({
        type: 'agent_list',
        persistent: [],
        ephemeral_templates: [],
        running: [],
        request_id: sent.request_id,
      }),
    );
    await promise;
    const lat = c.getLatencyMs();
    expect(lat).not.toBeNull();
    expect(lat!).toBeGreaterThanOrEqual(15);
    c.close();
  });

  it('reconnects with backoff after close', async () => {
    vi.useFakeTimers();
    const c = new CommandsClient({ initialBackoffMs: 100, maxBackoffMs: 1000 });
    c.init();
    const first = MockWebSocket.instances[0]!;
    first.simulateOpen();
    first.simulateClose();
    // Backoff 100ms
    await vi.advanceTimersByTimeAsync(100);
    expect(MockWebSocket.instances).toHaveLength(2);
    c.close();
    vi.useRealTimers();
  });

  it('close() rejects all pending commands', async () => {
    const c = new CommandsClient({ commandTimeoutMs: 5000 });
    c.init();
    const socket = MockWebSocket.instances[0]!;
    socket.simulateOpen();
    const p1 = c.send({ type: 'start_agent', id: 'a' });
    const p2 = c.send({ type: 'stop_agent', id: 'b' });
    c.close();
    await expect(p1).rejects.toThrow();
    await expect(p2).rejects.toThrow();
  });
});
