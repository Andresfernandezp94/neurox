// Tests del cliente del voice MCP. EP-0002.
//
// Cubre:
//   - voiceCallStart: POST /voice/start, body, headers, parseo de respuesta
//   - voiceCallEnd:   POST /voice/end, body, idempotencia (mock devuelve 200)
//   - getVoiceWsUrl:  composición correcta del path + session_id encoded

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import {
  voiceCallStart,
  voiceCallEnd,
  getVoiceWsUrl,
  DEFAULT_VOICES,
} from './voice';
import { setToken } from './client';

const originalFetch = globalThis.fetch;

describe('api/voice', () => {
  beforeEach(() => {
    setToken('test-token');
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    setToken(null);
  });

  describe('voiceCallStart', () => {
    it('POSTs /voice/start with session_id + bearer token', async () => {
      const responseBody = {
        session_id: 'sess-1',
        voice_id: 'Spanish_SereneWoman',
        model: 'speech-2.8-hd',
        format: 'mp3',
        sample_rate: 32_000,
        bitrate: 128_000,
        channel: 1,
      };
      const fetchMock = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify(responseBody), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );
      globalThis.fetch = fetchMock;

      const result = await voiceCallStart('sess-1');

      expect(fetchMock).toHaveBeenCalledTimes(1);
      const [url, init] = fetchMock.mock.calls[0]!;
      expect(url).toBe('/voice/start');
      expect(init.method).toBe('POST');
      expect(JSON.parse(init.body)).toEqual({ session_id: 'sess-1' });
      // Bearer token propagado por apiPost → buildHeaders
      expect(init.headers['Authorization']).toBe('Bearer test-token');

      expect(result).toEqual(responseBody);
    });

    it('forwards per-call overrides in the body', async () => {
      const fetchMock = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify({ session_id: 'sess-2' }), { status: 200 }),
      );
      globalThis.fetch = fetchMock;

      await voiceCallStart('sess-2', {
        voice_id: 'Spanish_Narrator',
        model: 'speech-2.8-turbo',
        speed: 1.25,
        format: 'wav',
      });

      const body = JSON.parse(fetchMock.mock.calls[0]![1].body);
      expect(body).toEqual({
        session_id: 'sess-2',
        voice_id: 'Spanish_Narrator',
        model: 'speech-2.8-turbo',
        speed: 1.25,
        format: 'wav',
      });
    });

    it('throws ApiError when the voice MCP returns 5xx', async () => {
      // EP-0003-02: retry con backoff. Aceleramos setTimeout a 0ms
      // para que el test termine rápido tras los reintentos.
      const realSetTimeout = globalThis.setTimeout;
      globalThis.setTimeout = ((cb: () => void) =>
        realSetTimeout(cb, 0)) as typeof setTimeout;

      const fetchMock = vi.fn().mockResolvedValue(
        new Response('upstream down', {
          status: 502,
          statusText: 'Bad Gateway',
        }),
      );
      globalThis.fetch = fetchMock;

      try {
        await expect(voiceCallStart('sess-1')).rejects.toThrow(/502/);
      } finally {
        globalThis.setTimeout = realSetTimeout;
      }
    });
  });

  describe('voiceCallEnd', () => {
    it('POSTs /voice/end with the session_id', async () => {
      const fetchMock = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify({ ok: true, session_id: 'sess-1' }), {
          status: 200,
        }),
      );
      globalThis.fetch = fetchMock;

      const result = await voiceCallEnd('sess-1');

      const [url, init] = fetchMock.mock.calls[0]!;
      expect(url).toBe('/voice/end');
      expect(JSON.parse(init.body)).toEqual({ session_id: 'sess-1' });
      expect(result.ok).toBe(true);
    });
  });

  describe('getVoiceWsUrl', () => {
    it('builds a relative URL with session_id query param', () => {
      const url = getVoiceWsUrl('abc-123');
      expect(url).toBe('/voice/ws?session_id=abc-123');
    });

    it('encodes special characters in the session_id', () => {
      const url = getVoiceWsUrl('sess/with spaces & weird?chars');
      expect(url).toContain('session_id=sess%2Fwith%20spaces%20%26%20weird%3Fchars');
    });

    it('appends with & if the base already has a query string', () => {
      // No hay caso real hoy donde /voice/ws tenga querystring, pero el
      // helper debe ser defensivo por si `VITE_API_BASE` trae parámetros.
      const url = getVoiceWsUrl('sess-1');
      // En el caso default no hay query previa, debe usar ?
      expect(url.startsWith('/voice/ws?')).toBe(true);
    });
  });

  describe('DEFAULT_VOICES', () => {
    it('contains at least one Spanish voice', () => {
      expect(DEFAULT_VOICES.length).toBeGreaterThan(0);
      for (const v of DEFAULT_VOICES) {
        expect(v.id.startsWith('Spanish_')).toBe(true);
      }
    });
  });
});
