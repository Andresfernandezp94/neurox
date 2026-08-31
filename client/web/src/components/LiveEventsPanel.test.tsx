// Tests del LiveEventsPanel — filtros y parsing de eventos.
// EP-0001-02.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LiveEventsPanel } from './LiveEventsPanel';

type Listener = (ev: Event | MessageEvent | CloseEvent) => void;

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  url: string;
  listeners: Record<string, Listener[]> = {};
  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }
  addEventListener(type: string, listener: Listener) {
    (this.listeners[type] = this.listeners[type] ?? []).push(listener);
  }
  removeEventListener() {}
  close() {
    this.listeners.close?.forEach((cb) => cb(new CloseEvent('close')));
  }
  emit(type: string, data?: string) {
    if (type === 'message' && data !== undefined) {
      this.listeners.message?.forEach((cb) =>
        cb(new MessageEvent('message', { data })),
      );
    } else if (type === 'open') {
      this.listeners.open?.forEach((cb) => cb(new Event('open')));
    }
  }
}

describe('LiveEventsPanel', () => {
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

  it('renders the empty state when no events have arrived', () => {
    render(<LiveEventsPanel />);
    expect(screen.getByText(/Waiting for events/)).toBeInTheDocument();
  });

  it('renders received events with type badges', async () => {
    const { findByText } = render(<LiveEventsPanel />);
    const socket = MockWebSocket.instances[0]!;
    socket.emit('open');
    socket.emit('message', JSON.stringify({ type: 'agent.started', id: 'default' }));
    socket.emit('message', JSON.stringify({ type: 'tool.invoked', name: 'shell' }));

    expect(await findByText(/agent.started/)).toBeInTheDocument();
    expect(await findByText(/tool.invoked/)).toBeInTheDocument();
  });

  it('has a filter checkbox for each type', () => {
    render(<LiveEventsPanel />);
    for (const t of ['agent', 'session', 'tool', 'approval', 'system', 'other']) {
      expect(screen.getByRole('checkbox', { name: new RegExp(`^${t}$`, 'i') })).toBeInTheDocument();
    }
  });

  it('shows a Clear button', () => {
    render(<LiveEventsPanel />);
    expect(screen.getByRole('button', { name: /Clear/ })).toBeInTheDocument();
  });
});
