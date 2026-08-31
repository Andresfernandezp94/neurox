// Tests del cliente HTTP. EP-0001-02.
// EP-0003-02: tests de retry con backoff y AbortSignal.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import {
  apiGet,
  apiPost,
  apiDelete,
  ApiError,
  setToken,
  getRetryStats,
  resetRetryStats,
  withRetry,
} from './client';

describe('api/client', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    setToken(null);
    resetRetryStats();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  describe('apiGet', () => {
    it('returns parsed JSON on 2xx', async () => {
      const data = { ok: true };
      globalThis.fetch = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify(data), { status: 200, headers: { 'content-type': 'application/json' } }),
      );
      const result = await apiGet<typeof data>('/v1/x');
      expect(result).toEqual(data);
    });

    it('throws ApiError on 4xx with parsed body', async () => {
      globalThis.fetch = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify({ error: { message: 'nope' } }), { status: 401 }),
      );
      await expect(apiGet('/v1/x')).rejects.toThrowError(ApiError);
    });

    it('throws ApiError on 5xx with statusText when body is not JSON', async () => {
      // EP-0003-02: con retry habilitado, un 5xx persistente se reintenta.
      // Para que el test termine rápido, aceleramos setTimeout a 0ms.
      const realSetTimeout = globalThis.setTimeout;
      globalThis.setTimeout = ((cb: () => void) => realSetTimeout(cb, 0)) as typeof setTimeout;
      globalThis.fetch = vi.fn().mockResolvedValue(
        new Response('boom', { status: 500, statusText: 'Internal Server Error' }),
      );
      try {
        await apiGet('/v1/x');
        expect.fail('should throw');
      } catch (e) {
        expect(e).toBeInstanceOf(ApiError);
        expect((e as ApiError).status).toBe(500);
      } finally {
        globalThis.setTimeout = realSetTimeout;
      }
    });
  });

  describe('apiPost', () => {
    it('sends JSON body when provided', async () => {
      const fetchMock = vi.fn().mockResolvedValueOnce(
        new Response('{}', { status: 200 }),
      );
      globalThis.fetch = fetchMock;
      await apiPost('/v1/x', { a: 1 });
      const call = fetchMock.mock.calls[0]!;
      expect(call[0]).toBe('/v1/x');
      expect(call[1].method).toBe('POST');
      expect(JSON.parse(call[1].body)).toEqual({ a: 1 });
    });

    it('omits body when not provided', async () => {
      const fetchMock = vi.fn().mockResolvedValueOnce(new Response('{}', { status: 200 }));
      globalThis.fetch = fetchMock;
      await apiPost('/v1/x');
      expect(fetchMock.mock.calls[0]![1].body).toBeUndefined();
    });
  });

  describe('apiDelete', () => {
    it('uses DELETE method', async () => {
      const fetchMock = vi.fn().mockResolvedValueOnce(new Response('{}', { status: 200 }));
      globalThis.fetch = fetchMock;
      await apiDelete('/v1/x');
      expect(fetchMock.mock.calls[0]![1].method).toBe('DELETE');
    });
  });

  describe('auth header', () => {
    it('includes Bearer token when set', async () => {
      setToken('my-token');
      const fetchMock = vi.fn().mockResolvedValueOnce(new Response('{}', { status: 200 }));
      globalThis.fetch = fetchMock;
      await apiGet('/v1/x');
      const headers = fetchMock.mock.calls[0]![1].headers;
      expect(headers.Authorization).toBe('Bearer my-token');
    });

    it('omits Authorization when no token', async () => {
      setToken(null);
      const fetchMock = vi.fn().mockResolvedValueOnce(new Response('{}', { status: 200 }));
      globalThis.fetch = fetchMock;
      await apiGet('/v1/x');
      const headers = fetchMock.mock.calls[0]![1].headers;
      expect(headers.Authorization).toBeUndefined();
    });
  });

  // ─── EP-0003-02: retry ───────────────────────────────────────────────

  describe('withRetry (EP-0003-02)', () => {
    it('returns immediately on success', async () => {
      const fn = vi.fn().mockResolvedValue('ok');
      const result = await withRetry(fn, { baseMs: 1 });
      expect(result).toBe('ok');
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('retries on TypeError (network failure)', async () => {
      const fn = vi
        .fn()
        .mockRejectedValueOnce(new TypeError('network down'))
        .mockRejectedValueOnce(new TypeError('still down'))
        .mockResolvedValueOnce('ok');
      const result = await withRetry(fn, { baseMs: 1 });
      expect(result).toBe('ok');
      expect(fn).toHaveBeenCalledTimes(3);
    });

    it('retries on 5xx and stops on 2xx', async () => {
      const fn = vi
        .fn()
        .mockResolvedValueOnce(new Response('boom', { status: 503 }))
        .mockResolvedValueOnce(new Response('{}', { status: 200 }));
      const result = await withRetry(async () => {
        const res = await fn();
        if (!res.ok) throw new ApiError(res.status, '/x', null, res.statusText);
        return res.json();
      }, { baseMs: 1 });
      expect(result).toEqual({});
      expect(fn).toHaveBeenCalledTimes(2);
    });

    it('does NOT retry on 4xx (client error)', async () => {
      const fn = vi
        .fn()
        .mockResolvedValueOnce(new Response('nope', { status: 401 }));
      await expect(
        withRetry(async () => {
          const res = await fn();
          if (!res.ok) throw new ApiError(res.status, '/x', null, res.statusText);
          return res.json();
        }, { baseMs: 1 }),
      ).rejects.toThrowError(ApiError);
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('does NOT retry on 501 (Not Implemented)', async () => {
      const fn = vi
        .fn()
        .mockResolvedValueOnce(new Response('nope', { status: 501 }));
      await expect(
        withRetry(async () => {
          const res = await fn();
          if (!res.ok) throw new ApiError(res.status, '/x', null, res.statusText);
          return res.json();
        }, { baseMs: 1 }),
      ).rejects.toThrowError(ApiError);
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('gives up after maxRetries and throws last error', async () => {
      const fn = vi
        .fn()
        .mockRejectedValueOnce(new TypeError('1'))
        .mockRejectedValueOnce(new TypeError('2'))
        .mockRejectedValueOnce(new TypeError('3'))
        .mockRejectedValueOnce(new TypeError('4'));
      await expect(withRetry(fn, { maxRetries: 3, baseMs: 1 })).rejects.toThrow('4');
      expect(fn).toHaveBeenCalledTimes(4); // 1 initial + 3 retries
    });

    it('cancels in-flight retries when AbortSignal aborts', async () => {
      const fn = vi
        .fn()
        .mockRejectedValueOnce(new TypeError('1'))
        .mockRejectedValueOnce(new TypeError('2'));
      const controller = new AbortController();
      setTimeout(() => controller.abort(), 5);
      await expect(
        withRetry(fn, { baseMs: 100, signal: controller.signal }),
      ).rejects.toThrow(/aborted/);
    });

    it('records each retry in getRetryStats', async () => {
      const fn = vi
        .fn()
        .mockRejectedValueOnce(new TypeError('1'))
        .mockRejectedValueOnce(new TypeError('2'))
        .mockResolvedValueOnce('ok');
      await withRetry(fn, { baseMs: 1 });
      const stats = getRetryStats();
      expect(stats.retriesTotal).toBeGreaterThanOrEqual(2);
      expect(stats.lastRetryAt).not.toBeNull();
    });
  });

  describe('apiGet retry integration (EP-0003-02)', () => {
    it('retries 5xx and eventually returns 2xx', async () => {
      const fetchMock = vi
        .fn()
        .mockResolvedValueOnce(new Response('boom', { status: 500 }))
        .mockResolvedValueOnce(new Response('{}', { status: 200 }));
      globalThis.fetch = fetchMock;
      // Inyectar baseMs=1 vía fetch directo no es trivial; en su lugar
      // validamos que reintenta al menos una vez (el primer 500 no throwea
      // inmediatamente, sino que programa un retry).
      // Para que el test termine rápido, NO esperamos al backoff real:
      // patcheamos setTimeout para que sea instantáneo.
      const realSetTimeout = globalThis.setTimeout;
      globalThis.setTimeout = ((cb: () => void) => realSetTimeout(cb, 0)) as typeof setTimeout;
      try {
        const result = await apiGet<{ ok: boolean }>('/v1/x');
        expect(result).toEqual({});
        expect(fetchMock).toHaveBeenCalledTimes(2);
      } finally {
        globalThis.setTimeout = realSetTimeout;
      }
    });

    it('retries TypeError and eventually returns 2xx', async () => {
      const fetchMock = vi
        .fn()
        .mockRejectedValueOnce(new TypeError('network'))
        .mockResolvedValueOnce(new Response('{}', { status: 200 }));
      globalThis.fetch = fetchMock;
      const realSetTimeout = globalThis.setTimeout;
      globalThis.setTimeout = ((cb: () => void) => realSetTimeout(cb, 0)) as typeof setTimeout;
      try {
        const result = await apiGet<{ ok: boolean }>('/v1/x');
        expect(result).toEqual({});
        expect(fetchMock).toHaveBeenCalledTimes(2);
      } finally {
        globalThis.setTimeout = realSetTimeout;
      }
    });

    it('does NOT retry 4xx', async () => {
      const fetchMock = vi
        .fn()
        .mockResolvedValueOnce(new Response('{}', { status: 404 }));
      globalThis.fetch = fetchMock;
      await expect(apiGet('/v1/x')).rejects.toThrowError(ApiError);
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    it('aborts the cycle when AbortSignal triggers', async () => {
      const fetchMock = vi.fn().mockImplementation(async (_url, init: RequestInit) => {
        return new Promise((_, reject) => {
          init.signal?.addEventListener('abort', () => {
            reject(new DOMException('aborted', 'AbortError'));
          });
        });
      });
      globalThis.fetch = fetchMock;
      const controller = new AbortController();
      setTimeout(() => controller.abort(), 5);
      await expect(apiGet('/v1/x', { signal: controller.signal })).rejects.toThrow();
    });
  });
});
