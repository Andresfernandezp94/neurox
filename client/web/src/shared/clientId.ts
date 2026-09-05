// Stable per-browser client identifier.
//
// Persisted to localStorage so that the same browser tab family
// (reload, restore) keeps the same `client_id`. Used by the daemon
// to partition the session list — web sessions never collide with
// sidebar sessions.
//
// Format: `web-<8-char-base36>` (sufficient entropy for one browser).
// We don't use a full UUID to keep the SSE payloads compact.

const STORAGE_KEY = "neurox.web.client_id";

let cached: string | null = null;

export function getWebClientId(): string {
  if (cached) return cached;
  if (typeof window === "undefined") return "web-server";
  try {
    const existing = window.localStorage.getItem(STORAGE_KEY);
    if (existing && /^web-[a-z0-9]{4,16}$/.test(existing)) {
      cached = existing;
      return existing;
    }
  } catch {
    // localStorage may be blocked (private mode); fall through.
  }
  const fresh = "web-" + Math.random().toString(36).slice(2, 10);
  try {
    window.localStorage.setItem(STORAGE_KEY, fresh);
  } catch {
    // Ignore: non-critical.
  }
  cached = fresh;
  return fresh;
}