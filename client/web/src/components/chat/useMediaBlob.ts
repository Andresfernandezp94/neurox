// useMediaBlob — fetch a media URL with Bearer auth and expose it as
// a stable blob: URL for <img>/<audio>/<video> src.
//
// Why a hook instead of putting the URL straight in the <img src>?
// The browser's request from <img src> cannot carry custom headers,
// so the daemon's Bearer auth would 401 the request. The fetch() API
// does let us add the Authorization header, and from the response blob
// we create an object URL that the <img>/<video>/<audio> can consume
// without any header gymnastics.
//
// EP-2026-08-19: extracted from SmartResult.MediaBlock to keep that
// renderer focused on presentation. The hook is shared by the three
// media kinds (image / audio / video) so they all benefit from the
// same loading / error states and the same cleanup discipline.

import { useEffect, useState } from "react";
import { buildApiUrl, getToken } from "../../api/client";

export type MediaStatus = "idle" | "loading" | "ready" | "error";

export interface MediaBlobState {
  /** blob: URL safe to assign to <img>/<video>/<audio> src. null until ready. */
  url: string | null;
  status: MediaStatus;
  /** Server-provided Content-Type, when known. Useful for <video> sources. */
  mime: string | null;
  /** Human-readable error message when status === "error". */
  error: string | null;
  /** Server-reported filename from Content-Disposition, when present. */
  filename: string | null;
}

/**
 * Fetch a media URL with Bearer auth and track its loading state.
 *
 * @param path absolute path under the daemon (e.g. "/v1/files/...")
 * @param enabled set to false to skip the fetch (e.g. while parent
 *   toggle is closed). When toggled back on, the fetch restarts.
 */
export function useMediaBlob(
  path: string | null | undefined,
  enabled: boolean,
): MediaBlobState {
  const [state, setState] = useState<MediaBlobState>({
    url: null,
    status: "idle",
    mime: null,
    error: null,
    filename: null,
  });

  useEffect(() => {
    if (!enabled || !path) {
      // Reset to idle when disabled; the previous blob URL is released
      // by the cleanup function of the previous effect run.
      setState({
        url: null,
        status: "idle",
        mime: null,
        error: null,
        filename: null,
      });
      return;
    }

    let cancelled = false;
    const ctrl = new AbortController();
    setState({
      url: null,
      status: "loading",
      mime: null,
      error: null,
      filename: null,
    });

    (async () => {
      try {
        const url = buildApiUrl(path);
        const headers: Record<string, string> = {};
        const token = getToken();
        if (token) headers["Authorization"] = `Bearer ${token}`;

        const res = await fetch(url, { headers, signal: ctrl.signal });
        if (!res.ok) {
          // Mirror the same 401-clears-token behaviour as the rest of the
          // API client so a stale token doesn't keep the spinner up
          // forever. 401 also bubbles to the global auth-expired event
          // (handled in api/client.ts) via window event below.
          if (res.status === 401) {
            try {
              sessionStorage.removeItem("neurox_token");
            } catch {
              /* sessionStorage may be unavailable */
            }
            window.dispatchEvent(new CustomEvent("neurox:auth-expired"));
          }
          // EP-2026-08-19: el daemon suele responder con un body útil
          // (ej. `{"error": "file not found"}` o texto plano). Lo
          // capturamos para que el mensaje al usuario no sea solo
          // "404 Not Found" — eso deja sin pistas sobre qué pidió mal.
          let body = "";
          try {
            const ct = res.headers.get("Content-Type") ?? "";
            if (ct.includes("application/json")) {
              const j = await res.json();
              body =
                (typeof j === "object" && j && "error" in j
                  ? String((j as { error: unknown }).error)
                  : "") ||
                (typeof j === "object" && j && "message" in j
                  ? String((j as { message: unknown }).message)
                  : "");
            } else {
              body = (await res.text()).slice(0, 200);
            }
          } catch {
            /* body no parsea — dejamos el status solo */
          }
          if (cancelled) return;
          setState({
            url: null,
            status: "error",
            mime: null,
            error: body
              ? `${res.status} ${res.statusText}: ${body}`
              : `${res.status} ${res.statusText}`,
            filename: null,
          });
          return;
        }

        const blob = await res.blob();
        if (cancelled) return;

        const objectUrl = URL.createObjectURL(blob);
        const mime = res.headers.get("Content-Type");
        // Content-Disposition may carry filename="..."; if so prefer it
        // for the <a download> attribute.
        const disp = res.headers.get("Content-Disposition") ?? "";
        const fnMatch = /filename="?([^";]+)"?/i.exec(disp);
        const filename = fnMatch && fnMatch[1] ? fnMatch[1] : null;

        setState({
          url: objectUrl,
          status: "ready",
          mime,
          error: null,
          filename,
        });
      } catch (e) {
        if (cancelled) return;
        const msg =
          e instanceof Error
            ? e.name === "AbortError"
              ? ""
              : e.message
            : String(e);
        setState({
          url: null,
          status: "error",
          mime: null,
          error: msg,
          filename: null,
        });
      }
    })();

    return () => {
      cancelled = true;
      ctrl.abort();
    };
  }, [path, enabled]);

  // Release the blob URL on unmount or when path changes.
  useEffect(() => {
    return () => {
      if (state.url) URL.revokeObjectURL(state.url);
    };
  }, [state.url]);

  return state;
}
