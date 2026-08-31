// Tests del hook useVoiceCall. EP-0002.
//
// Mockeamos todas las APIs del navegador que el hook toca:
//   - WebSocket (clase)
//   - AudioContext (clase)
//   - navigator.mediaDevices.getUserMedia
//   - window.SpeechRecognition (clase)
//
// Cada test corre en aislamiento; el `beforeEach` re-instala los mocks
// por si un test los muta.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useVoiceCall } from './useVoiceCall';

// ─── Mocks de las APIs del navegador ────────────────────────────────────────

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static OPEN = 1;
  static CLOSED = 3;
  static CONNECTING = 0;
  readyState = 0; // CONNECTING
  url: string;
  onopen: ((ev: Event) => void) | null = null;
  onclose: ((ev: CloseEvent) => void) | null = null;
  onmessage: ((ev: MessageEvent) => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;
  sent: string[] = [];
  private listeners = new Map<string, Set<EventListener>>();

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }
  send(data: string) {
    this.sent.push(data);
  }
  close(code = 1000) {
    this.readyState = MockWebSocket.CLOSED;
    const ev = { code } as CloseEvent;
    this.onclose?.(ev);
    this.dispatchEvent(new Event('close'));
  }
  addEventListener(type: string, listener: EventListener) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(listener);
  }
  removeEventListener() { /* noop */ }
  dispatchEvent(event: Event): boolean {
    const set = this.listeners.get(event.type);
    if (set) for (const l of set) l(event);
    return true;
  }

  /** Test helper: dispara onopen manualmente. */
  open() {
    this.readyState = MockWebSocket.OPEN;
    const ev = new Event('open');
    this.onopen?.(ev);
    this.dispatchEvent(ev);
  }
  /** Test helper: manda un mensaje JSON al cliente. */
  receiveJSON(obj: unknown) {
    this.onmessage?.({ data: JSON.stringify(obj) } as MessageEvent);
  }
  /** Test helper: manda un mensaje binario al cliente. */
  receiveBinary(buf: ArrayBuffer) {
    this.onmessage?.({ data: buf } as MessageEvent);
  }
}

class MockAudioBufferSourceNode {
  buffer: AudioBuffer | null = null;
  started = false;
  connect() { /* noop */ }
  start() { this.started = true; }
}

class MockAnalyserNode {
  fftSize = 1024;
  getFloatTimeDomainData(_buf: Float32Array) { /* noop */ }
  connect() { /* noop */ }
}

class MockAudioContext {
  state: 'running' | 'closed' | 'suspended' = 'running';
  destination = {};
  decodeAudioData = vi.fn().mockResolvedValue({} as AudioBuffer);
  createMediaStreamSource = vi.fn().mockReturnValue({ connect: vi.fn() });
  createAnalyser = vi.fn().mockReturnValue(new MockAnalyserNode());
  createBufferSource = vi.fn().mockReturnValue(new MockAudioBufferSourceNode());
  resume = vi.fn().mockResolvedValue(undefined);
  close = vi.fn().mockResolvedValue(undefined);
}

class MockSpeechRecognition {
  static instances: MockSpeechRecognition[] = [];
  continuous = false;
  interimResults = false;
  lang = '';
  onresult: ((ev: unknown) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  onend: ((ev: Event) => void) | null = null;
  started = false;
  aborted = false;
  constructor() {
    MockSpeechRecognition.instances.push(this);
  }
  start() { this.started = true; }
  stop() { /* noop */ }
  abort() { this.aborted = true; }

  /** Test helper: simula un resultado final. */
  emitFinal(text: string) {
    this.onresult?.({
      resultIndex: 0,
      results: [{ 0: { transcript: text, confidence: 1 }, isFinal: true, length: 1 }] as unknown as ArrayLike<unknown>,
    } as unknown);
  }
  /** Test helper: simula un resultado interim. */
  emitInterim(text: string) {
    this.onresult?.({
      resultIndex: 0,
      results: [{ 0: { transcript: text, confidence: 0 }, isFinal: false, length: 1 }] as unknown as ArrayLike<unknown>,
    } as unknown);
  }
}

const mockGetUserMedia = vi.fn().mockResolvedValue({
  getTracks: () => [{ stop: vi.fn() }],
} as unknown as MediaStream);

// ─── Setup / teardown ───────────────────────────────────────────────────────

beforeEach(() => {
  MockWebSocket.instances.length = 0;
  MockSpeechRecognition.instances.length = 0;
  // Sustituir APIs del navegador. Usamos vi.stubGlobal para WebSocket
  // porque en Node 22+ hay un WebSocket nativo (undici) que pisa el
  // globalThis.WebSocket; assignar la propiedad no es suficiente.
  vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
  // En Node 22+ hay un WebSocket nativo (undici) que se cuelga con URLs
  // relativas y no acepta mockeo vía vi.stubGlobal. Forzamos con
  // defineProperty para garantizar que el global sea nuestro mock.
  Object.defineProperty(globalThis, 'WebSocket', {
    configurable: true,
    writable: true,
    value: MockWebSocket,
  });
  (window as unknown as { AudioContext: typeof AudioContext }).AudioContext =
    MockAudioContext as unknown as typeof AudioContext;
  (window as unknown as { webkitSpeechRecognition?: unknown }).webkitSpeechRecognition =
    MockSpeechRecognition as unknown as unknown;
  // jsdom expone `navigator.mediaDevices` con un getter, por eso un
  // assignment simple no pega. Forzamos con defineProperty.
  Object.defineProperty(navigator, 'mediaDevices', {
    configurable: true,
    value: { getUserMedia: mockGetUserMedia },
  });
  // jsdom no implementa requestAnimationFrame de forma síncrona.
  // El hook llama a requestAnimationFrame(tick) recursivamente desde
  // dentro de tick → con un mock que llama cb sincrónicamente
  // entraríamos en loop infinito. Por eso el mock corre cb UNA sola
  // vez y devuelve un id que cancelAnimationFrame acepta.
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb) => {
    setTimeout(() => cb(performance.now()), 0);
    return 1;
  });
  vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// ─── Tests ──────────────────────────────────────────────────────────────────

/**
 * Helper: arranca el hook y dispara el open event del WS resultante.
 *
 * `start()` es async y bloquea hasta que el WS abre. En los tests
 * tenemos que disparar el open manualmente. Esta helper se encarga de:
 *
 *   1. Llamar a start() sin awaitearla (capturando la promise).
 *   2. Esperar a que `new WebSocket()` haya sido invocado.
 *   3. Disparar `ws.open()` y awaitear la promise del start().
 *
 * Uso:
 *   await startAndOpenHook(() => result.current.start());
 */
async function startAndOpenHook(
  startFn: () => Promise<void>,
): Promise<void> {
  let startPromise: Promise<void> | null = null;
  await act(async () => {
    startPromise = startFn();
    // Damos yield para que el flujo async avance hasta `new WebSocket()`.
    for (let i = 0; i < 5 && MockWebSocket.instances.length === 0; i++) {
      await Promise.resolve();
    }
  });
  const ws = MockWebSocket.instances[0];
  expect(ws, 'WebSocket was not created during start()').toBeDefined();
  await act(async () => {
    ws!.open();
    await startPromise!;
  });
}

describe('useVoiceCall', () => {
  it('starts in idle state with no error', () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );
    expect(result.current.state).toBe('idle');
    expect(result.current.level).toBe(0);
    expect(result.current.lastError).toBeNull();
    expect(result.current.partialTranscript).toBe('');
    expect(result.current.sttSupported).toBe(true);
  });

  it('rejects start() when sessionId is null', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: null, autoSend: true }),
    );
    await act(async () => {
      await result.current.start();
    });
    expect(result.current.state).toBe('error');
    expect(result.current.lastError).toMatch(/no hay session_id/);
  });

  it('transitions idle → connecting → ready on start()', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );

    // Disparamos start() sin esperar — el sync setState('connecting')
    // se aplica de inmediato, y la parte async (getMedia + openWs +
    // startRecognition) queda corriendo.
    let startPromise: Promise<void> | null = null;
    act(() => {
      startPromise = result.current.start();
    });

    expect(result.current.state).toBe('connecting');

    // Esperamos a que el WS sea creado (openWs corre después de getMedia).
    await act(async () => {
      for (let i = 0; i < 5 && MockWebSocket.instances.length === 0; i++) {
        await Promise.resolve();
      }
      const ws = MockWebSocket.instances[0]!;
      expect(ws).toBeDefined();
      expect(ws.url).toContain('session_id=sess-1');
      ws.open();
      await startPromise!;
    });

    expect(result.current.state).toBe('ready');
  });

  it('toggle() calls start() from idle and stop() when running', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );

    await startAndOpenHook(() => result.current.start());
    expect(result.current.state).toBe('ready');

    await act(async () => {
      result.current.toggle();
      await Promise.resolve();
    });
    expect(result.current.state).toBe('idle');
  });

  it('STT final result with autoSend=true sends {type:"text"} to WS', async () => {
    const onUserTranscript = vi.fn();
    const { result } = renderHook(() =>
      useVoiceCall({
        sessionId: 'sess-1',
        autoSend: true,
        onUserTranscript,
      }),
    );

    await startAndOpenHook(() => result.current.start());

    await act(async () => {
      MockSpeechRecognition.instances[0]!.emitFinal('hola mundo');
      await Promise.resolve();
    });

    const ws = MockWebSocket.instances[0]!;
    expect(ws.sent.some((s) => s.includes('"type":"text"') && s.includes('hola mundo'))).toBe(true);
    expect(onUserTranscript).toHaveBeenCalledWith('hola mundo');
  });

  it('STT final result with autoSend=false does NOT send to WS but calls callback', async () => {
    const onUserTranscript = vi.fn();
    const { result } = renderHook(() =>
      useVoiceCall({
        sessionId: 'sess-1',
        autoSend: false,
        onUserTranscript,
      }),
    );

    await startAndOpenHook(() => result.current.start());

    await act(async () => {
      MockSpeechRecognition.instances[0]!.emitFinal('dictar esto');
      await Promise.resolve();
    });

    const ws = MockWebSocket.instances[0]!;
    expect(ws.sent.some((s) => s.includes('dictar esto'))).toBe(false);
    expect(onUserTranscript).toHaveBeenCalledWith('dictar esto');
  });

  it('server status frame updates state', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );

    await startAndOpenHook(() => result.current.start());
    expect(result.current.state).toBe('ready');

    await act(async () => {
      MockWebSocket.instances[0]!.receiveJSON({ type: 'status', state: 'thinking' });
      await Promise.resolve();
    });
    expect(result.current.state).toBe('thinking');

    await act(async () => {
      MockWebSocket.instances[0]!.receiveJSON({ type: 'status', state: 'speaking' });
      await Promise.resolve();
    });
    expect(result.current.state).toBe('speaking');

    await act(async () => {
      MockWebSocket.instances[0]!.receiveJSON({ type: 'status', state: 'ready' });
      await Promise.resolve();
    });
    expect(result.current.state).toBe('ready');
  });

  it('server error frame sets state to error and surfaces message', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );

    await startAndOpenHook(() => result.current.start());

    await act(async () => {
      MockWebSocket.instances[0]!.receiveJSON({ type: 'error', message: 'upstream down' });
      await Promise.resolve();
    });

    expect(result.current.state).toBe('error');
    expect(result.current.lastError).toBe('upstream down');
  });

  it('assistant transcript from server triggers onAssistantTranscript callback', async () => {
    const onAssistantTranscript = vi.fn();
    const { result } = renderHook(() =>
      useVoiceCall({
        sessionId: 'sess-1',
        autoSend: true,
        onAssistantTranscript,
      }),
    );

    await startAndOpenHook(() => result.current.start());

    await act(async () => {
      MockWebSocket.instances[0]!.receiveJSON({
        type: 'transcript',
        role: 'assistant',
        text: 'Hola, ¿en qué te ayudo?',
      });
      await Promise.resolve();
    });

    expect(onAssistantTranscript).toHaveBeenCalledWith('Hola, ¿en qué te ayudo?');
  });

  it('binary frame triggers audio decode + playback (no throw)', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );

    await startAndOpenHook(() => result.current.start());

    await act(async () => {
      MockWebSocket.instances[0]!.receiveBinary(new ArrayBuffer(8));
      await Promise.resolve();
    });

    // Smoke: la llamada no tira. El decodeAudioData del mock resuelve
    // con un objeto vacío, suficiente para no romper la call.
    expect(result.current.state).toBe('ready');
  });

  it('stop() sends {type:"stop"} to WS and transitions to idle', async () => {
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );

    await startAndOpenHook(() => result.current.start());

    await act(async () => {
      result.current.stop();
      await Promise.resolve();
    });

    expect(result.current.state).toBe('idle');
    const ws = MockWebSocket.instances[0]!;
    expect(ws.sent.some((s) => s.includes('"type":"stop"'))).toBe(true);
  });

  it('reports sttSupported=false when no SpeechRecognition is on window', () => {
    delete (window as unknown as { SpeechRecognition?: unknown }).SpeechRecognition;
    delete (window as unknown as { webkitSpeechRecognition?: unknown }).webkitSpeechRecognition;
    const { result } = renderHook(() =>
      useVoiceCall({ sessionId: 'sess-1', autoSend: true }),
    );
    expect(result.current.sttSupported).toBe(false);
  });
});
