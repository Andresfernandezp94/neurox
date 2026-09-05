// Cliente HTTP genérico del admin.
// Lee el Bearer token de sessionStorage si existe, y lo agrega como header.
// Lanza ApiError cuando la respuesta no es 2xx.
// EP-0003-02: agrega retry con backoff en 5xx y TypeError, soporte de AbortSignal.

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly url: string,
    public readonly body: unknown,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

const TOKEN_KEY = 'neurox_token';

// VITE_API_BASE se inyecta en build-time por Vite. Si está vacío, las URLs
// son relativas al origen actual (útil en dev: Vite proxy reenvía /v1/*
// al daemon). En producción hay que setearlo al dominio del backend (e.g.
// "https://mcp.example.com") para evitar same-origin y que la SPA pegue
// contra Pages en vez del daemon.
//
// En producción (import.meta.env.PROD), un build sin VITE_API_BASE
// falla loudly — antes el comportamiento "silencioso" hacía que la SPA
// se deployara correcta y simplemente no conectara con nada (los /v1/*
// se resolvían contra CF Pages, que devuelve el index.html HTML, no
// JSON, derivando en 'Unexpected token <' al parsear).
export function getApiBase(): string {
  const base = (import.meta.env?.VITE_API_BASE as string | undefined) ?? '';
  if (!base && import.meta.env.PROD) {
    throw new Error(
      "[neurox-web] VITE_API_BASE is not set. Building for production " +
        "without VITE_API_BASE makes the SPA send REST + WS calls to its " +
        "own origin (Cloudflare Pages), where there is no /v1/* surface. " +
        "Restart the build with VITE_API_BASE pointing at the daemon, " +
        "e.g. `task deploy:pages VITE_API_BASE=https://mcp.neurox.pro`.",
    );
  }
  return base;
}

function resolveUrl(path: string): string {
  const base = getApiBase();
  // Si path ya es absoluta (http/https) la dejo tal cual.
  if (/^https?:\/\//i.test(path)) return path;
  return `${base}${path}`;
}

// Exportado para callers que necesitan streaming (no encaja en apiPost).
export function buildApiUrl(path: string): string {
  return resolveUrl(path);
}

export function getToken(): string | null {
  try {
    return sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string | null): void {
  try {
    if (token) sessionStorage.setItem(TOKEN_KEY, token);
    else sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    // sessionStorage puede no estar disponible (modo privado, etc).
  }
}

export function buildHeaders(): HeadersInit {
  const h: Record<string, string> = { 'Content-Type': 'application/json' };
  const token = getToken();
  if (token) h['Authorization'] = `Bearer ${token}`;
  return h;
}

async function handleResponse<T>(res: Response, url: string): Promise<T> {
  if (!res.ok) {
    let body: unknown = null;
    try {
      body = await res.json();
    } catch {
      // body no es JSON
    }
    const msg = (body as { error?: { message?: string } } | null)?.error?.message
      ?? `${res.status} ${res.statusText}`;
    // EP-0007: 401 from the daemon means our token is invalid/expired.
    // Clear it so the SPA shows the LoginScreen on the next render.
    if (res.status === 401 && !url.includes('/v1/auth/login')) {
      setToken(null);
      // Notify listeners (App.tsx) to re-render.
      window.dispatchEvent(new CustomEvent('neurox:auth-expired'));
    }
    throw new ApiError(res.status, url, body, msg);
  }
  return res.json() as Promise<T>;
}

// ─── Retry (EP-0003-02) ──────────────────────────────────────────────────

export interface RetryStats {
  retriesTotal: number;
  lastRetryAt: number | null;
}

const retryStats: RetryStats = {
  retriesTotal: 0,
  lastRetryAt: null,
};

export function getRetryStats(): RetryStats {
  return { ...retryStats };
}

function recordRetry(): void {
  retryStats.retriesTotal += 1;
  retryStats.lastRetryAt = Date.now();
}

export function resetRetryStats(): void {
  retryStats.retriesTotal = 0;
  retryStats.lastRetryAt = null;
}

export interface RetryOptions {
  /** Máximo de reintentos (default 3). Total attempts = 1 + maxRetries. */
  maxRetries?: number;
  /** Backoff base en ms (default 1000). */
  baseMs?: number;
  /** Cap de backoff en ms (default 16000). */
  maxBackoffMs?: number;
  /** AbortSignal para cancelar el ciclo de reintentos. */
  signal?: AbortSignal;
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException('aborted', 'AbortError'));
      return;
    }
    const t = setTimeout(resolve, ms);
    if (signal) {
      const onAbort = () => {
        clearTimeout(t);
        reject(new DOMException('aborted', 'AbortError'));
      };
      signal.addEventListener('abort', onAbort, { once: true });
    }
  });
}

function isRetryable(err: unknown): boolean {
  if (err instanceof ApiError) {
    // 501 Not Implemented: explícitamente no retry (es semántico).
    if (err.status === 501) return false;
    return err.status >= 500;
  }
  // fetch tira TypeError en network failures.
  if (err instanceof TypeError) return true;
  return false;
}

export async function withRetry<T>(
  fn: () => Promise<T>,
  opts: RetryOptions = {},
): Promise<T> {
  const maxRetries = opts.maxRetries ?? 3;
  const baseMs = opts.baseMs ?? 1000;
  const maxBackoffMs = opts.maxBackoffMs ?? 16_000;
  const signal = opts.signal;

  let lastErr: unknown;
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    if (signal?.aborted) {
      throw new DOMException('aborted', 'AbortError');
    }
    try {
      return await fn();
    } catch (e) {
      lastErr = e;
      if (!isRetryable(e)) throw e;
      if (attempt === maxRetries) throw e;
      const backoff = Math.min(baseMs * Math.pow(2, attempt), maxBackoffMs);
      recordRetry();
      await sleep(backoff, signal);
    }
  }
  // Inalcanzable; el loop siempre termina con throw o return.
  throw lastErr;
}

// ─── API pública ─────────────────────────────────────────────────────────

export interface RequestOptions {
  signal?: AbortSignal;
}

export async function apiGet<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const url = resolveUrl(path);
  return withRetry(
    async () => {
      const res = await fetch(url, { method: 'GET', headers: buildHeaders(), signal: opts.signal });
      return handleResponse<T>(res, url);
    },
    { signal: opts.signal },
  );
}

export async function apiPost<T>(path: string, body?: unknown, opts: RequestOptions = {}): Promise<T> {
  const url = resolveUrl(path);
  return withRetry(
    async () => {
      const res = await fetch(url, {
        method: 'POST',
        headers: buildHeaders(),
        body: body !== undefined ? JSON.stringify(body) : undefined,
        signal: opts.signal,
      });
      return handleResponse<T>(res, url);
    },
    { signal: opts.signal },
  );
}

export async function apiDelete<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const url = resolveUrl(path);
  return withRetry(
    async () => {
      const res = await fetch(url, { method: 'DELETE', headers: buildHeaders(), signal: opts.signal });
      return handleResponse<T>(res, url);
    },
    { signal: opts.signal },
  );
}

export async function apiPatch<T>(path: string, body?: unknown, opts: RequestOptions = {}): Promise<T> {
  const url = resolveUrl(path);
  return withRetry(
    async () => {
      const res = await fetch(url, {
        method: 'PATCH',
        headers: buildHeaders(),
        body: body !== undefined ? JSON.stringify(body) : undefined,
        signal: opts.signal,
      });
      return handleResponse<T>(res, url);
    },
    { signal: opts.signal },
  );
}

export async function apiPut<T>(path: string, body?: unknown, opts: RequestOptions = {}): Promise<T> {
  const url = resolveUrl(path);
  return withRetry(
    async () => {
      const res = await fetch(url, {
        method: 'PUT',
        headers: buildHeaders(),
        body: body !== undefined ? JSON.stringify(body) : undefined,
        signal: opts.signal,
      });
      return handleResponse<T>(res, url);
    },
    { signal: opts.signal },
  );
}
