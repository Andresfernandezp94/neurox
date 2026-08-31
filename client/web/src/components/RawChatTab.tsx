// RawChatTab — debug panel que muestra la respuesta CRUDA del LLM
// (sin filtrar, sin `think` strip, sin remapeo de tool_calls).
//
// EP-RAW-CHAT (2026-08-12): corre contra `POST /v1/chat/raw` que el
// daemon proxy-ea a llmd, que a su vez pide los bytes crudos del
// SSE upstream (MiniMax / OpenAI-compat / Anthropic según el
// provider activo).
//
// Dos vistas toggleables:
//   - **JSON** (default): cada chunk tal cual llega del provider, con
//     field path colapsado para que quepa en pantalla
//   - **Formatted**: thinking en bloque colapsable, content en flujo
//     de texto plano, tool_call/tool_result como cards separadas

import { useCallback, useEffect, useRef, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";

type ViewMode = "json" | "formatted";

interface RawChunk {
  /** Index incremental (0, 1, 2…) — útil para ordenarlos */
  index: number;
  /** Texto crudo del chunk SSE (sin el prefijo "data: " ni el newline) */
  raw: string;
  /** Parseado si fue JSON válido; null si no parseó */
  parsed: unknown | null;
  /** Para Anthropic: nombre del evento cuando viene "event: ..." */
  eventName?: string;
  /** Marca de tiempo relativa al primer chunk (ms) */
  tMs: number;
}

export function RawChatTab(): React.JSX.Element {
  const [prompt, setPrompt] = useState("Di hola en un emoji");
  const [chunks, setChunks] = useState<RawChunk[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<ViewMode>("json");
  const [provider, setProvider] = useState<string>("");
  const [model, setModel] = useState<string>("");
  const startRef = useRef<number>(0);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Auto-scroll al fondo cuando llega un chunk nuevo.
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [chunks.length]);

  // Carga provider/model activos al montar.
  useEffect(() => {
    void (async () => {
      try {
        const r = await fetch("/v1/providers/active");
        if (!r.ok) return;
        const v = (await r.json()) as { provider_id?: string; model?: string };
        if (v.provider_id) setProvider(v.provider_id);
        if (v.model) setModel(v.model);
      } catch {
        // ignore — los defaults funcionan igual
      }
    })();
  }, []);

  const send = useCallback(async () => {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    setError(null);
    setChunks([]);
    startRef.current = performance.now();
    const ctrl = new AbortController();
    abortRef.current = ctrl;
    try {
      const res = await fetch("/v1/chat/raw", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          text: prompt,
          provider_id: provider || undefined,
          model: model || undefined,
        }),
        signal: ctrl.signal,
      });
      if (!res.ok || !res.body) {
        setError(`HTTP ${res.status} ${res.statusText}`);
        setBusy(false);
        return;
      }
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buf = "";
      let pendingEvent = "";
      let i = 0;
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        let nl: number;
        while ((nl = buf.indexOf("\n")) !== -1) {
          const line = buf.slice(0, nl).replace(/\r$/, "");
          buf = buf.slice(nl + 1);
          const trimmed = line.trim();
          if (!trimmed) {
            pendingEvent = "";
            continue;
          }
          if (trimmed.startsWith("event:")) {
            pendingEvent = trimmed.slice(6).trim();
            continue;
          }
          if (!trimmed.startsWith("data:")) continue;
          const payload = trimmed.slice(5).trim();
          if (payload === "[DONE]") {
            // OpenAI-compat cierra con [DONE] — terminamos.
            setBusy(false);
            return;
          }
          let parsed: unknown | null = null;
          try {
            parsed = JSON.parse(payload);
          } catch {
            // Mantener raw aunque no sea JSON válido.
          }
          setChunks((prev) => [
            ...prev,
            {
              index: i++,
              raw: payload,
              parsed,
              eventName: pendingEvent || undefined,
              tMs: Math.round(performance.now() - startRef.current),
            },
          ]);
          pendingEvent = "";
        }
      }
      setBusy(false);
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  }, [prompt, busy, provider, model]);

  const cancel = useCallback(() => {
    abortRef.current?.abort();
    setBusy(false);
  }, []);

  const clear = useCallback(() => {
    setChunks([]);
    setError(null);
  }, []);

  return (
    <div className="raw-chat">
      <Card title="Raw chat" data-testid="raw-chat-card">
        <div className="raw-chat__controls">
          <input
            className="raw-chat__input"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Escribe un mensaje…"
            disabled={busy}
            data-testid="raw-chat-input"
          />
          <button
            type="button"
            className="raw-chat__btn raw-chat__btn--primary"
            onClick={send}
            disabled={busy || !prompt.trim()}
            data-testid="raw-chat-send"
          >
            {busy ? "Sending…" : "Send"}
          </button>
          {busy && (
            <button
              type="button"
              className="raw-chat__btn"
              onClick={cancel}
              data-testid="raw-chat-cancel"
            >
              Cancel
            </button>
          )}
          <button
            type="button"
            className="raw-chat__btn"
            onClick={clear}
            disabled={chunks.length === 0}
            data-testid="raw-chat-clear"
          >
            Clear
          </button>
          <div className="raw-chat__provider">
            <label>
              Provider:
              <input
                className="raw-chat__provider-input"
                value={provider}
                onChange={(e) => setProvider(e.target.value)}
                placeholder="active"
                data-testid="raw-chat-provider"
              />
            </label>
            <label>
              Model:
              <input
                className="raw-chat__provider-input"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="active"
                data-testid="raw-chat-model"
              />
            </label>
          </div>
          <div className="raw-chat__view-toggle">
            <button
              type="button"
              className={`raw-chat__btn ${view === "json" ? "raw-chat__btn--active" : ""}`}
              onClick={() => setView("json")}
              data-testid="raw-chat-view-json"
            >
              JSON ({chunks.length})
            </button>
            <button
              type="button"
              className={`raw-chat__btn ${view === "formatted" ? "raw-chat__btn--active" : ""}`}
              onClick={() => setView("formatted")}
              data-testid="raw-chat-view-formatted"
            >
              Formatted
            </button>
          </div>
        </div>

        {error && (
          <ErrorBanner variant="error">
            <span data-testid="raw-chat-error">{error}</span>
          </ErrorBanner>
        )}
      </Card>

      <div className="raw-chat__stream" ref={scrollRef} data-testid="raw-chat-stream">
        {chunks.length === 0 && !busy && (
          <p className="muted">
            Send a prompt to see the raw provider SSE stream. Chunks
            appear in arrival order. Use the view toggle to switch
            between raw JSON and a friendly formatted parse.
          </p>
        )}
        {chunks.map((c) => (
          <div key={c.index} className="raw-chat__chunk" data-testid={`raw-chat-chunk-${c.index}`}>
            <div className="raw-chat__chunk-meta">
              <span className="raw-chat__chunk-idx">#{c.index}</span>
              <span className="raw-chat__chunk-t">+{c.tMs}ms</span>
              {c.eventName && (
                <span className="raw-chat__chunk-event">event: {c.eventName}</span>
              )}
            </div>
            {view === "json" ? (
              <pre className="raw-chat__chunk-raw">{c.raw}</pre>
            ) : (
              <FormattedChunk parsed={c.parsed} eventName={c.eventName} />
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * Render amigable de un chunk parseado. Funciona tanto con el formato
 * OpenAI-compat (`choices[0].delta.content` / `.reasoning` /
 * `.tool_calls`) como con Anthropic (`type: content_block_delta`,
 * `message_start`, etc.).
 */
function FormattedChunk({ parsed, eventName }: { parsed: unknown | null; eventName?: string }): React.JSX.Element {
  if (parsed === null) {
    return <pre className="raw-chat__chunk-raw">(no JSON parseable)</pre>;
  }
  const obj = parsed as Record<string, unknown>;

  // Anthropic event-style
  if (typeof obj.type === "string" && obj.type.startsWith("content_block_")) {
    const delta = obj.delta as { text?: string; type?: string } | undefined;
    if (delta?.text) {
      return <div className="raw-chat__chunk-content">+{delta.text}</div>;
    }
    return (
      <pre className="raw-chat__chunk-raw">
        [{eventName ?? obj.type}] {JSON.stringify(obj, null, 2)}
      </pre>
    );
  }
  if (typeof obj.type === "string" && (obj.type === "message_start" || obj.type === "message_delta" || obj.type === "message_stop")) {
    return (
      <div className="raw-chat__chunk-meta-line">
        [{eventName ?? obj.type}]
      </div>
    );
  }

  // OpenAI-compat
  const choices = obj.choices as Array<{ delta?: { content?: string; reasoning?: string; tool_calls?: unknown[] }; finish_reason?: string }> | undefined;
  const choice0 = choices?.[0];
  const delta = choice0?.delta;
  if (delta?.reasoning) {
    return <div className="raw-chat__chunk-think">thinking: {delta.reasoning}</div>;
  }
  if (delta?.content) {
    return <div className="raw-chat__chunk-content">+{delta.content}</div>;
  }
  if (delta?.tool_calls && Array.isArray(delta.tool_calls) && delta.tool_calls.length > 0) {
    return (
      <div className="raw-chat__chunk-tool">
        tool_call: {JSON.stringify(delta.tool_calls)}
      </div>
    );
  }
  if (choice0?.finish_reason) {
    return (
      <div className="raw-chat__chunk-finish">
        finish_reason: {choice0.finish_reason}
      </div>
    );
  }
  // Fallback
  return <pre className="raw-chat__chunk-raw">{JSON.stringify(obj, null, 2)}</pre>;
}
