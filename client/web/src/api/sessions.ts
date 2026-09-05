import { apiGet, apiPost, apiPut, apiDelete, buildApiUrl, buildHeaders } from './client';
import { getWebClientId } from '../shared/clientId';
import type { SessionsResponse, MessagesResponse } from '../types';

// Per-turn request id so the daemon can route concurrent streams on the
// same session without cross-wiring events (D3). Reset on each `send`.
export function newRequestId(): string {
  return "req_" + Math.random().toString(36).slice(2, 10) + Date.now().toString(36);
}

export function listSessions(): Promise<SessionsResponse> {
  // Pass client_id so the daemon filters to sessions owned by this
  // browser. Without this we'd see sidebar sessions too.
  return apiGet<SessionsResponse>(`/v1/sessions?client_id=${encodeURIComponent(getWebClientId())}`);
}

export function createSession(agent_id: string): Promise<{ session_id: string; agent_id: string }> {
  return apiPost('/v1/sessions', { agent_id, client_id: getWebClientId() });
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

export function sendMessage(
  id: string,
  agent_id: string,
  text: string,
  provider_id: string,
  model: string,
): Promise<unknown> {
  return apiPost(`/v1/sessions/${encodeURIComponent(id)}/messages`, {
    agent_id,
    text,
    provider_id,
    model,
    client_id: getWebClientId(),
  });
}

/**
 * Stream a message response chunk-by-chunk via the SSE endpoint.
 *
 * Calls `onChunk` for each parsed JSON object as it arrives. Resolves when
 * the stream ends with `[DONE]`. Rejects on network errors or non-2xx status.
 *
 * `requestId` (optional): when provided, the client filters out any chunk
 * whose `request_id` field doesn't match. This is what prevents
 * cross-wiring when two `send()` calls overlap on the same session
 * (rare in the UI because `useChatStream` aborts the previous one,
 * but possible during teardown).
 *
 * SSE framing is parsed properly: events are separated by `\n\n`,
 * multi-line `data:` continuations are joined, comments (`:`) are
 * ignored. Earlier versions split on `\n` which broke for chunks
 * whose JSON payload contained literal newlines (W1).
 */
export async function streamMessage(
  id: string,
  agent_id: string,
  text: string,
  provider_id: string,
  model: string,
  onChunk: (chunk: unknown) => void,
  signal?: AbortSignal,
  requestId?: string,
): Promise<void> {
  const url = buildApiUrl(`/v1/sessions/${encodeURIComponent(id)}/messages/stream`);
  const res = await fetch(url, {
    method: "POST",
    // buildHeaders() agrega Authorization: Bearer <jwt> cuando hay token
    // (mismo helper que usa apiPost/apiGet/etc). Sin esto, el backend
    // responde 401 en /v1/* (rutas protegidas por JwtAuthLayer).
    headers: buildHeaders(),
    body: JSON.stringify({
      agent_id,
      text,
      provider_id,
      model,
      client_id: getWebClientId(),
      // EP-D3: el daemon lo refleja en cada chunk para que el filtro
      // per-turn del frontend (requestId vs chunk.request_id) pueda
      // descartar chunks cruzados entre streams concurrentes.
      ...(requestId ? { request_id: requestId } : {}),
    }),
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

    // Split on SSE event terminator (\n\n). Each event is one or
    // more `field: value\n` lines ending with a blank line.
    let frameEnd: number;
    while ((frameEnd = buffer.indexOf("\n\n")) !== -1) {
      const rawFrame = buffer.slice(0, frameEnd);
      buffer = buffer.slice(frameEnd + 2);
      processFrame(rawFrame, onChunk, requestId);
    }
  }

  // Flush trailing partial frame if any.
  const trailing = buffer.replace(/\r$/, "").trim();
  if (trailing) {
    processFrame(trailing, onChunk, requestId);
  }
}

function processFrame(
  rawFrame: string,
  onChunk: (chunk: unknown) => void,
  requestId?: string,
): void {
  // Concatenate multi-line `data:` fields per SSE spec.
  let dataLines: string[] = [];
  for (const rawLine of rawFrame.split("\n")) {
    const line = rawLine.replace(/\r$/, "");
    if (!line) continue;
    if (line.startsWith(":")) continue; // comment
    const idx = line.indexOf(":");
    const field = idx >= 0 ? line.slice(0, idx) : line;
    let value = idx >= 0 ? line.slice(idx + 1) : "";
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "data") dataLines.push(value);
  }
  if (dataLines.length === 0) return;
  const payload = dataLines.join("\n");
  if (payload === "[DONE]") return; // end of stream
  let parsed: unknown;
  try {
    parsed = JSON.parse(payload);
  } catch {
    return; // malformed JSON — ignore
  }
  // D3: drop chunks whose request_id doesn't match this turn.
  if (requestId && typeof parsed === "object" && parsed !== null) {
    const obj = parsed as Record<string, unknown>;
    if (typeof obj.request_id === "string" && obj.request_id !== requestId) {
      return;
    }
  }
  onChunk(parsed);
}