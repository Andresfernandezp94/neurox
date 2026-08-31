import { apiGet, apiPost, apiPut, apiDelete, buildApiUrl } from './client';
import type { SessionsResponse, MessagesResponse } from '../types';

export function listSessions(): Promise<SessionsResponse> {
  return apiGet<SessionsResponse>('/v1/sessions');
}

export function createSession(agent_id: string): Promise<{ session_id: string; agent_id: string }> {
  return apiPost('/v1/sessions', { agent_id });
}

export function getSessionMessages(id: string): Promise<MessagesResponse> {
  return apiGet<MessagesResponse>(`/v1/sessions/${encodeURIComponent(id)}/messages`);
}

export function renameSession(id: string, name: string): Promise<unknown> {
  return apiPut(`/v1/sessions/${encodeURIComponent(id)}/rename`, { name });
}

export function cancelSession(id: string): Promise<unknown> {
  // POST /v1/sessions/:id/cancel — stop the running agent but keep the
  // session record (the message log stays). Use `deleteSession` to
  // remove the session entirely.
  return apiPost(`/v1/sessions/${encodeURIComponent(id)}/cancel`);
}

export function deleteSession(id: string): Promise<unknown> {
  // DELETE /v1/sessions/:id — kill the agent and remove the session
  // record from the store.
  return apiDelete(`/v1/sessions/${encodeURIComponent(id)}`);
}

export function sendMessage(id: string, agent_id: string, text: string): Promise<unknown> {
  return apiPost(`/v1/sessions/${encodeURIComponent(id)}/messages`, {
    agent_id,
    text,
  });
}

/**
 * Stream a message response chunk-by-chunk via the SSE endpoint.
 *
 * Calls `onChunk` for each parsed JSON object as it arrives. Resolves when
 * the stream ends with `[DONE]`. Rejects on network errors or non-2xx status.
 *
 * Backend event shapes:
 *   {"type":"thinking","text":"..."}
 *   {"type":"content","text":"..."}
 *   {"type":"error","message":"..."}
 *   "[DONE]"
 */
export async function streamMessage(
  id: string,
  agent_id: string,
  text: string,
  onChunk: (chunk: unknown) => void,
  signal?: AbortSignal,
): Promise<void> {
  const url = buildApiUrl(`/v1/sessions/${encodeURIComponent(id)}/messages/stream`);
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ agent_id, text }),
    signal,
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${res.statusText}`);
  }
  if (!res.body) {
    throw new Error("No response body");
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });

    let newlineIdx: number;
    while ((newlineIdx = buffer.indexOf("\n")) !== -1) {
      const rawLine = buffer.slice(0, newlineIdx);
      buffer = buffer.slice(newlineIdx + 1);
      const line = rawLine.replace(/\r$/, "").trim();
      if (!line) continue;
      if (!line.startsWith("data:")) continue;
      const payload = line.slice(5).trim();
      if (payload === "[DONE]") return;
      try {
        onChunk(JSON.parse(payload));
      } catch {
        // ignore malformed JSON
      }
    }
  }

  // Flush any trailing buffer as a final chunk.
  const trailing = buffer.replace(/\r$/, "").trim();
  if (trailing) {
    try {
      onChunk(JSON.parse(trailing));
    } catch {
      // ignore
    }
  }
}
