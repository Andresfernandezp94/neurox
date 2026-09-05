// useChatStream — encapsulates the lifecycle of a chat SSE stream.
//
// Pure I/O wrapper. For each chunk it parses the wire format and
// delegates immediately to the caller's `onChunk` callback (which
// applies `applyStreamChunk` — the same pure utility the remote WS
// path uses in `useChatTabs.ts`). Single source of truth: one reducer
// runs per chunk, owned by the caller.
//
// What stays here:
//   - AbortController across renders (cancel-safe)
//   - Stable `onChunk` ref so `send`'s dep array stays minimal
//   - Cleanup on unmount so the fetch + reader don't leak
//   - Lifecycle surfaced as `status` and `error`

import { useCallback, useEffect, useRef, useState } from "react";
import { streamMessage, newRequestId } from "../api/sessions";
import { parseStreamChunk } from "../components/chat/streaming/chunk";
import type { StreamChunk } from "../components/chat/streaming/chunk";

export type ChatStreamStatus = "idle" | "streaming" | "error";

export interface UseChatStreamOptions {
  /** Backend session id (required). */
  sessionId: string;
  /** Agent id used by the stream endpoint. */
  agentId: string;
  /** LLM provider the user picked for the current session (or null to fall
   *  back to the daemon's configured default). Shipped on every message. */
  providerId: string | null;
  /** Model id within `providerId`. Shipped with `providerId`. */
  model: string | null;
  /**
   * Called for every successfully-parsed chunk. The caller applies it
   * to its own message state via `applyStreamChunk(msg, chunk, acc)`
   * — the same utility the remote WS path uses — so both paths emit
   * the same collapsed timeline.
   */
  onChunk: (chunk: StreamChunk) => void;
}

export interface UseChatStreamReturn {
  /**
   * Start a stream for `text`. The promise resolves when the stream
   * ends normally. Rejects with the underlying error on fetch / network
   * failures, or resolves if `cancel()` is called.
   */
  send: (text: string) => Promise<void>;
  /** Abort the in-flight stream (if any). Idempotent. */
  cancel: () => void;
  status: ChatStreamStatus;
  /** Last error message (cleared on the next successful send). */
  error: string | null;
}

export function useChatStream({
  sessionId,
  agentId,
  providerId,
  model,
  onChunk,
}: UseChatStreamOptions): UseChatStreamReturn {
  const [status, setStatus] = useState<ChatStreamStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  // Abort controller for the in-flight stream. Null when idle.
  const abortRef = useRef<AbortController | null>(null);

  // Stable refs to the latest callbacks. Lets `send` keep a minimal dep
  // array while still always invoking the freshest closures (important
  // because `onChunk` typically captures tab-level state).
  const onChunkRef = useRef(onChunk);
  useEffect(() => {
    onChunkRef.current = onChunk;
  }, [onChunk]);

  // Cleanup: abort in-flight stream when the hook unmounts so we don't
  // leak the fetch + reader.
  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  const send = useCallback(
    async (text: string): Promise<void> => {
      if (!text.trim()) return;
      setError(null);
      setStatus("streaming");

      const abort = new AbortController();
      abortRef.current = abort;

      // EP-2026-08-15 (debug): wire-format inspection. Activate by
      // running in DevTools:
      //   localStorage.setItem('debug_stream', '1')
      // and reproducing a stream. The console will print both the raw
      // JSON of every chunk and the chunk type after the parser
      // runs (or `null` if the parser rejected it). Off by default.
      const DEBUG_STREAM =
        typeof window !== "undefined" &&
        window.localStorage?.getItem("debug_stream") === "1";

      try {
        // D3: tag this turn so the SSE reader drops cross-wired chunks
        // (only relevant if a previous stream is still being torn down).
        const requestId = newRequestId();
        await streamMessage(
          sessionId,
          agentId,
          text,
          providerId ?? "",
          model ?? "",
          (raw) => {
            if (DEBUG_STREAM) {
              console.log("[stream-chunk] raw:", JSON.stringify(raw));
            }
            const chunk = parseStreamChunk(raw);
            if (DEBUG_STREAM) {
              console.log(
                "[stream-chunk] parsed:",
                chunk ? chunk.type : "null (rejected)",
              );
            }
            if (!chunk) return; // unknown / malformed — ignored
            onChunkRef.current(chunk);
          },
          abort.signal,
          requestId,
        );
        setStatus("idle");
      } catch (e) {
        // AbortError is the expected fallout of `cancel()`. Don't
        // surface it as a user-facing error.
        if ((e as Error).name === "AbortError") {
          setStatus("idle");
          return;
        }
        setError((e as Error).message);
        setStatus("error");
        throw e;
      } finally {
        abortRef.current = null;
      }
    },
    [sessionId, agentId, providerId, model],
  );
  const cancel = useCallback(() => {
    if (!abortRef.current) return;
    abortRef.current.abort();
    // `send` will catch the AbortError and setStatus("idle").
    // Don't clear `error` here — a previous stream error should stay
    // visible until the user retries.
  }, []);

  return { send, cancel, status, error };
}
