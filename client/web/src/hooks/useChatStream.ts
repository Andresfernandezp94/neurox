// useChatStream — encapsulates the lifecycle of a chat SSE stream.
//
// The reducer (`chat/streaming/reducer.ts`) is the state machine; this
// hook is the lifecycle wrapper around it:
//
//   - opens the connection (via `streamMessage`)
//   - parses each chunk (`parseStreamChunk`)
//   - applies the reducer (`streamReducer`)
//   - mirrors the resulting state back to the caller (via `onState`)
//   - exposes a `cancel` that aborts in-flight streams
//   - surfaces lifecycle as `status` and `error`
//
// Why a hook? Three things the reducer/parser don't know how to do:
//   1. hold an `AbortController` across renders (cancel-safe)
//   2. reuse the latest `onState` callback without re-creating `send`
//      every render (so the dep array of `useCallback` stays small)
//   3. clean up if the component unmounts mid-stream

import { useCallback, useEffect, useRef, useState } from "react";
import { streamMessage, newRequestId } from "../api/sessions";
import { parseStreamChunk } from "../components/chat/streaming/chunk";
import {
  initStreamState,
  streamReducer,
  type StreamState,
} from "../components/chat/streaming/reducer";

export type ChatStreamStatus = "idle" | "streaming" | "error";

export interface UseChatStreamOptions {
  /** Backend session id (required). */
  sessionId: string;
  /** Agent id used by the stream endpoint. */
  agentId: string;
  /**
   * Called for every successfully-parsed chunk with the resulting
   * full state. Receives a fresh immutable reference per chunk so the
   * caller can pass it straight to setState (no need to copy).
   */
  onState: (state: StreamState) => void;
}

export interface UseChatStreamReturn {
  /**
   * Start a stream for `text`. The promise resolves with the final
   * accumulated `StreamState` once the stream ends normally (so the
   * caller can compute metrics like token counts / totalText).
   * Rejects with the underlying error on fetch / network failures,
   * or resolves to the partial state if `cancel()` is called.
   */
  send: (text: string) => Promise<StreamState>;
  /** Abort the in-flight stream (if any). Idempotent. */
  cancel: () => void;
  status: ChatStreamStatus;
  /** Last error message (cleared on the next successful send). */
  error: string | null;
}

export function useChatStream({
  sessionId,
  agentId,
  onState,
}: UseChatStreamOptions): UseChatStreamReturn {
  const [status, setStatus] = useState<ChatStreamStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  // Abort controller for the in-flight stream. Null when idle.
  const abortRef = useRef<AbortController | null>(null);

  // Stable ref to the latest `onState`. Lets `send` keep a minimal dep
  // array while still always invoking the freshest callback closure
  // (important because `onState` typically captures tab-level state).
  const onStateRef = useRef(onState);
  useEffect(() => {
    onStateRef.current = onState;
  }, [onState]);

  // Cleanup: abort in-flight stream when the hook unmounts so we don't
  // leak the fetch + reader.
  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  const send = useCallback(
    async (text: string): Promise<StreamState> => {
      if (!text.trim()) {
        return initStreamState();
      }
      setError(null);
      setStatus("streaming");

      const abort = new AbortController();
      abortRef.current = abort;

      let streamState = initStreamState();

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
            streamState = streamReducer(streamState, chunk);
            onStateRef.current(streamState);
          },
          abort.signal,
          requestId,
        );
        setStatus("idle");
        return streamState;
      } catch (e) {
        // AbortError is the expected fallout of `cancel()`. Don't
        // surface it as a user-facing error.
        if ((e as Error).name === "AbortError") {
          setStatus("idle");
          // Return whatever had accumulated up to the cancel; the
          // caller decides whether to discard it or close it out
          // with whatever text streamed in.
          return streamState;
        }
        setError((e as Error).message);
        setStatus("error");
        throw e;
      } finally {
        abortRef.current = null;
      }
    },
    [sessionId, agentId],
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
