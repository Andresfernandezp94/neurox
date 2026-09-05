// ChatPanel — agent-studio inspired: timeline layout, typewriter streaming,
// thinking as collapsible timeline node (not a bubble).
// EP-0024: tabs arriba (vía useChatTabs) — múltiples sesiones en paralelo.
// Refactor: dividido en ChatHeader + ChatMain + ChatFooter.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { VoiceCallOverlay } from "./VoiceCallOverlay";
import { useChatTabs } from "../hooks/useChatTabs";
import { useDefaultAgentId } from "../hooks/useDefaultAgentId";
import {
  createSession,
  cancelSession,
  getSessionMessages,
  listSessions,
} from "../api/sessions";
import { getDefaultAgentStatus, type DefaultAgentResponse } from "../api/default";
import { countTranscriptMatches } from "./chat/searchTranscript";
import type { Message, MessageMetrics } from "../types";
import { PanelToggle } from "../shared/components/PanelToggle";
import { useChatStream } from "../hooks/useChatStream";
import { ChatHeader } from "./ChatHeader";
import { ChatMain } from "./ChatMain";
import { ChatFooter } from "./ChatFooter";

function estimateTokens(text: string): number {
  return Math.max(1, Math.ceil(text.length / 4));
}

function computeMetrics(durationMs: number, text: string): MessageMetrics {
  const tokens = estimateTokens(text);
  const tokensPerSec = durationMs > 0 ? tokens / (durationMs / 1000) : 0;
  return { durationMs, tokens, tokensPerSec };
}

// ─── Main Component ──────────────────────────────────────────────────────────

export interface ChatPanelProps {}

export function ChatPanel(_: ChatPanelProps = {}) {
  // EP-0024: el agent de la sesión se selecciona por tab (mismo patrón
  // que el modelo). `useDefaultAgentId` lee de `state.agents` (poblado
  // por `/v1/agents` en mount). Si el daemon está caído o todavía no
  // reportó agentes, devuelve `null` → mandamos string vacío y el
  // daemon responde con `agent_id required` (más claro que
  // `agent not found: <stale-id>`).
  const daemonDefaultAgentId = useDefaultAgentId();
  const {
    tabs,
    activeTab,
    activeId,
    createTab,
    createTabFromSession,
    closeTab,
    selectTab,
    renameTab,
    updateTab,
    updateTabSummary,
    setStreamingMetrics,
  } = useChatTabs(daemonDefaultAgentId);
  const inProcessDefaultAgent =
    activeTab?.sessionAgent ?? daemonDefaultAgentId ?? "";

  const [input, setInput] = useState("");
  // EP-2026-08-15: stream lifecycle moved out into `useChatStream`.
  // We derive `isStreaming` from the hook's status (instead of a
  // local `busy` state) so cancel / dispose / abort semantics live
  // in one place. The refs the hook needs (`streamingAssistantIdRef`
  // and `streamingTabIdRef`) are declared near the other refs further
  // down — kept adjacent to where the hook itself is initialized.
  const [error, setError] = useState<string | null>(null);
  const [showHistory, setShowHistory] = useState(false);
  // EP-0002: voice call overlay visibility. Toggle desde el botón 📞
  // en la barra de tools del chat. Independiente del MicButton inline —
  // son dos affordances distintas para el mismo MCP.
  const [voiceOverlayOpen, setVoiceOverlayOpen] = useState(false);
  // EP-0026-UX: fullscreen toggle via Fullscreen API. Sincroniza con
  // `fullscreenchange` para que Esc u otro disparador externo se refleje
  // en el state.
  const fullscreenContainerRef = useRef<HTMLDivElement | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(
    typeof document !== "undefined" && !!document.fullscreenElement,
  );
  useEffect(() => {
    const handler = () => setIsFullscreen(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", handler);
    return () => document.removeEventListener("fullscreenchange", handler);
  }, []);

  // EP-2026-08-15: fallback de scroll para fullscreen + teclado en Android.
  // En modo fullscreen algunos browsers no disparan `visualViewport.resize`
  // cuando se abre el teclado, así que el shell no se reajusta y el footer
  // queda "atrapado" en su posición original. Este effect observa el focus
  // del textarea y dispara un scrollIntoView del footer cuando se gana
  // focus. Smooth (no jump), block-end para alinear con el bottom.
  useEffect(() => {
    const chatRoot = fullscreenContainerRef.current;
    if (!chatRoot) return;
    const textarea = chatRoot.querySelector<HTMLTextAreaElement>(".chat__textarea");
    const footer = chatRoot.querySelector<HTMLElement>(".chat__footer");
    if (!textarea || !footer) return;

    const onFocus = () => {
      // Pequeño defer para que el teclado ya esté abierto cuando scrolleamos.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          footer.scrollIntoView({ behavior: "smooth", block: "end" });
        }),
      );
    };
    textarea.addEventListener("focus", onFocus);
    return () => textarea.removeEventListener("focus", onFocus);
  }, []);
  const handleToggleFullscreen = useCallback(() => {
    const el = fullscreenContainerRef.current;
    if (!el) return;
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    } else if (el.requestFullscreen) {
      el.requestFullscreen().catch(() => {});
    }
  }, []);
  // EP-2026-08-15: el transcript se ancla arriba (scrollTop = 0)
  // cuando llega contenido nuevo — los mensajes crecen hacia abajo
  // pero el viewport NO sigue el último mensaje. El usuario puede
  // scrollear manualmente al fondo si quiere ver lo más reciente.
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  // Mirror of `activeTab` so the SSE onChunk callback (which we don't
  // want to re-create on every tab change) can read the current
  // active tab without being captured stale.
  const activeTabRef = useRef(activeTab);
  useEffect(() => {
    activeTabRef.current = activeTab;
  }, [activeTab]);
  // Mirror of `tabs` so `handleSend`'s `await sendStream(text)` can
  // read the LATEST tab state on resume. The closure's `tabs` is the
  // value at handleSend creation — by the time the SSE resolves, the
  // shell created by the first chunk may have been REPLACED by the
  // canonical id via the WS MessageAppended handler. Looking up by
  // `streamingMessageId` (the shell id) would miss it.
  const tabsRef = useRef<typeof tabs>(tabs);
  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);
  // EP-2026-09-05: streamingMessageId is the id of the assistant
  // message currently being streamed for the active tab. ChatMain
  // uses it to pass `isStreaming` per-message to MessageRow so the
  // caret animates only on the streaming row.
  //
  // Chunks now flow ONLY via the `/v1/events` WS (single source of
  // truth), so ChatPanel no longer tracks the id imperatively. We
  // derive it: while the turn is in flight, the streaming assistant is
  // the LAST assistant row in the transcript (the shell created by the
  // WS handler on the first chunk). Derivation happens below, after
  // `messages` and `isStreaming` are in scope.

  // EP-2026-09-05: the SSE stream is used ONLY to trigger the turn on
  // the backend. Chunks are applied EXCLUSIVELY via the `/v1/events`
  // WS in `useChatTabs` (single source of truth, ordered by the
  // daemon's per-session `seq`). So `onChunk` here is a no-op — it
  // must NOT apply chunks, otherwise the assistant message would be
  // double-updated (once via SSE, once via WS broadcast), which is the
  // exact duplication/truncation bug this refactor removes.
  const {
    send: sendStream,
    cancel: cancelStream,
    status: streamStatus,
  } = useChatStream({
    sessionId: activeTab?.sessionId ?? "",
    agentId: inProcessDefaultAgent,
    providerId: activeTab?.sessionModel?.provider_id ?? null,
    model: activeTab?.sessionModel?.model ?? null,
    // No-op: the WS path owns chunk application. See the comment above.
    onChunk: useCallback(() => {}, []),
  });
  const isStreaming = streamStatus === "streaming";

  // EP-0028 HMR-fix: declared here (before any useMemo/effect that
  // references it) so the value is in scope when the function body
  // runs top-to-bottom. Previously it was declared at the bottom of
  // the component, which put it in the TDZ for the `searchTotal`
  // memo above and caused a `ReferenceError: Cannot access 'messages'
  // before initialization` on every render — triggering HMR reload
  // loops and the `EPIPE` errors in the WS proxy.
  const messages = activeTab?.messages ?? [];
  const sessionId = activeTab?.sessionId ?? null;
  const sessionModel = activeTab?.sessionModel ?? null;

  // Derived: the assistant row currently streaming (see the note near
  // the top of the component). While a turn is in flight, it's the
  // last assistant message in the transcript — the shell the WS
  // handler created on the first chunk. `null` when idle so no caret
  // renders.
  const streamingMessageId = useMemo<number | null>(() => {
    if (!isStreaming) return null;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i]!.role === "assistant") return messages[i]!.id;
    }
    return null;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isStreaming, messages]);

  // EP-0026-01 R2: auto-create a backend session for the active tab
  // if it doesn't have one yet. The <textarea> stays disabled with
  // `placeholder="Connecting…"` until sessionId is set, so we
  // trigger the create call as soon as the tab becomes active.
  // The cancel flag avoids a race where the tab changes (or the
  // component unmounts) before the response arrives.
  useEffect(() => {
    if (!activeTab || activeTab.sessionId) return;
    // Wait until the daemon's `/v1/agents` has loaded so we don't
    // race with the StoreProvider's fetch (which would send `""` and
    // get a confusing `agent_id required` from the daemon).
    if (!daemonDefaultAgentId) return;
    let cancelled = false;
    (async () => {
      try {
        // Resolve the agent id at send-time: prefer the tab's
        // explicit choice, fall back to the daemon's in-process
        // agent from `state.agents`. (Sentinel empty string was
        // already handled by the early-return above.)
        const agentId = activeTab.sessionAgent || daemonDefaultAgentId;
        const resp = await createSession(agentId);
        if (!cancelled && resp.session_id) {
          updateTab(activeTab.id, { sessionId: resp.session_id });
        }
      } catch (e) {
        if (!cancelled) {
          // Leave the textarea disabled; user can close+recreate
          // the tab. The error is logged for debugging.
          // eslint-disable-next-line no-console
          console.error("createSession failed", e);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [activeTab?.id, activeTab?.sessionId, activeTab?.sessionAgent, daemonDefaultAgentId, updateTab]);

  // Auto-resize the textarea to its content height on every input
  // change. Reset to "auto" first so scrollHeight reflects the
  // real content height (otherwise the textarea never shrinks
  // back when lines are removed). Capped by the CSS max-height
  // (10rem); when the cap is reached, the CSS overflow-y: auto
  // rule kicks in and the textarea scrolls internally.
  useLayoutEffect(() => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [input]);

  // Close history on click outside
  useEffect(() => {
    if (!showHistory) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      // EP-0024: the SessionList root uses `className="session-list ..."`
      // (not "chat-history"); the sidebar wrapper also gets `chat-history`
      // via its container in the JSX below. Either ancestor counts.
      if (
        target.closest(".chat-history") ||
        target.closest(".session-list")
      ) {
        return;
      }
      setShowHistory(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showHistory]);

  // DefaultAgent status polling (real context info). EP-0024: also surfaces
  // workspace cwd, git_branch, and the sandbox scope the agent is
  // currently receiving so the user can verify what's in effect.
  const [defaultAgent, setDefaultAgent] = useState<DefaultAgentResponse | null>(null);
  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const status = await getDefaultAgentStatus();
        if (!cancelled) setDefaultAgent(status);
      } catch {
        /* default status is best-effort; the bar will just render what
           it has. */
      }
    };
    void poll();
    const interval = setInterval(() => void poll(), 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [activeTab?.messages.length]);

  // Reset input when switching tabs.
  useEffect(() => {
    setInput("");
    setError(null);
  }, [activeId]);

  // ── Search-in-chat (per tab) ─────────────────────────────────────────
  // Query, active match index and the total live on the tab itself so
  // closing+opening the same tab keeps the markers consistent. The
  // `total` is computed via `countTranscriptMatches` whenever messages
  // or the query change.
  const searchQuery = activeTab?.searchQuery ?? "";
  const searchActive = activeTab?.searchActive ?? 0;
  const searchTotal = useMemo(
    () => countTranscriptMatches(messages, searchQuery),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [messages, searchQuery],
  );
  // Clamp the active index when total shrinks (e.g. user edited a
  // message and the last match disappeared).
  useEffect(() => {
    if (searchTotal === 0) {
      if (searchActive !== 0) updateTab(activeTab!.id, { searchActive: 0 });
      return;
    }
    if (searchActive >= searchTotal) {
      updateTab(activeTab!.id, { searchActive: searchTotal - 1 });
    }
  }, [searchTotal, searchActive, activeTab, updateTab]);

  const handleSearchQueryChange = useCallback(
    (q: string) => {
      if (!activeTab) return;
      // Reset to first match when the query changes.
      updateTab(activeTab.id, { searchQuery: q, searchActive: 0 });
    },
    [activeTab, updateTab],
  );
  const handleSearchActiveChange = useCallback(
    (next: number) => {
      if (!activeTab) return;
      updateTab(activeTab.id, { searchActive: next });
    },
    [activeTab, updateTab],
  );
  const handleSearchClear = useCallback(() => {
    if (!activeTab) return;
    updateTab(activeTab.id, { searchQuery: "", searchActive: 0 });
  }, [activeTab, updateTab]);

  // Scroll the active match into view when it changes. The <mark>
  // elements carry `data-match-index={n}` and the active one has
  // `data-testid="chat-search-mark-active"` (set by renderHighlight).
  useEffect(() => {
    if (searchTotal === 0) return;
    const el = document.querySelector(
      '[data-testid="chat-search-mark-active"]',
    ) as HTMLElement | null;
    if (el) el.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [searchActive, searchTotal]);
  // ────────────────────────────────────────────────────────────────────

  // Resolve the persisted model for the given session by querying the
  // sessions list (EP-0017-02 R5 surfaces provider_id/model per summary).
  // EP-0028: takes an explicit tabId so it works for both the active
  // tab and a freshly-created tab from `createTabFromSession`.
  const refreshSessionModel = useCallback(async (sid: string, tabId: string) => {
    try {
      const resp = await listSessions();
      const summary = resp.sessions.find((s) => s.session_id === sid);
      if (summary?.provider_id && summary?.model) {
        updateTab(tabId, { sessionModel: { provider_id: summary.provider_id, model: summary.model } });
      } else {
        updateTab(tabId, { sessionModel: null });
      }
      // Apply the backend's summary as the tab title if the user
      // hasn't renamed it manually.
      if (summary?.summary) {
        updateTabSummary(tabId, summary.summary);
      }
    } catch {
      // Non-fatal.
    }
  }, [updateTab, updateTabSummary]);

  // Load an existing session (called from history). EP-0028: the
  // optional summary is the backend's title for the session — we
  // create a fresh tab bound to it so the History sidebar and the
  // tab header stay in sync. If a tab for this session already
  // exists, `createTabFromSession` just focuses it.
  const loadSession = useCallback(async (id: string, summary: string | null) => {
    const tab = await createTabFromSession(id, summary);
    try {
      const res = await getSessionMessages(id);
      const loaded: Message[] = (res.messages ?? []).map((m, i) => ({
        id: i + 1,
        session_id: id,
        role: m.role as Message["role"],
        content: m.content,
        ts: m.ts,
      }));
      updateTab(tab.id, { messages: loaded });
      await refreshSessionModel(id, tab.id);
    } catch (e) {
      setError(`Failed to load session: ${(e as Error).message}`);
    }
  }, [createTabFromSession, refreshSessionModel, updateTab]);

  // Cross-device chat history hydration. When the active tab has a
  // sessionId but no messages loaded yet (typical when the tab
  // appeared via WS sync or initial hydrate on a different device),
  // pull the full transcript from the daemon so all devices see the
  // same chat. Deduped via `loadedSessions` ref to avoid re-fetching
  // when the user re-selects the same tab.
  const loadedSessions = useRef<Set<string>>(new Set());
  useEffect(() => {
    const sid = activeTab?.sessionId;
    const tabId = activeTab?.id;
    if (!sid || !tabId) return;
    if (activeTab.messages.length > 0) {
      loadedSessions.current.add(sid);
      return;
    }
    if (loadedSessions.current.has(sid)) return;
    loadedSessions.current.add(sid);
    (async () => {
      try {
        const res = await getSessionMessages(sid);
        const loaded: Message[] = (res.messages ?? []).map((m, i) => ({
          id: i + 1,
          session_id: sid,
          role: m.role as Message["role"],
          content: m.content,
          ts: m.ts,
          thinking: m.thinking ?? undefined,
        }));
        // BUGFIX (cross-device realtime): apply the snapshot with the
        // FUNCTIONAL form and guard against clobbering messages that
        // arrived over the `/v1/events` WS while this fetch was in
        // flight. If another device sent a prompt (or the assistant
        // started streaming) between the `messages.length === 0`
        // check above and this resolve, `prev.messages` is no longer
        // empty — keeping the live copy is correct (it's fresher than
        // this snapshot, which may predate the new turn). We also
        // never overwrite a non-empty transcript with an empty
        // snapshot (a just-created session whose first row hasn't
        // been persisted yet).
        updateTab(tabId, (prev) => {
          if (prev.messages.length > 0) return prev;
          if (loaded.length === 0) return prev;
          return { ...prev, messages: loaded };
        });
        await refreshSessionModel(sid, tabId);
      } catch (e) {
        setError(`Failed to load session: ${(e as Error).message}`);
        // Allow retry on next tab switch.
        loadedSessions.current.delete(sid);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab?.id, activeTab?.sessionId]);

  // EP-0028: pull the backend's auto-summary for the given session
  // and mirror it onto the tab header (unless the user already
  // renamed it). Called after `handleSend` so the first user
  // message's preview shows up as the tab title.
  const refreshTabSummary = useCallback(async (sid: string, tabId: string) => {
    try {
      const resp = await listSessions();
      const s = resp.sessions.find((x) => x.session_id === sid);
      if (s?.summary) {
        updateTabSummary(tabId, s.summary);
      }
    } catch {
      // Non-fatal; the tab keeps its current title.
    }
  }, [updateTabSummary]);

// EP-2026-08-15: chat normal (`flex-direction: column`). El
// transcript crece hacia abajo y el viewport se ancla al fondo
// del scroll range para que el último mensaje SIEMPRE sea
// visible (al entrar a la sesión y al llegar contenido nuevo).
//
// Runs as a layout effect (before paint) over `activeId` and
// `messages.length` so it covers tab switches AND async
// loadSession resolves.
const lastMsg = messages[messages.length - 1];
const lastMsgIsStreaming =
  !!lastMsg && lastMsg.role === "assistant" && !lastMsg.metrics;
const lastMsgSig = lastMsg
  ? `${lastMsg.content.length}|${lastMsg.timeline?.length ?? 0}|${lastMsg.toolLog?.length ?? 0}|${lastMsg.approvals?.length ?? 0}`
  : "0";
useLayoutEffect(() => {
  // Search match scroll-into-view wins while a search query is active.
  if (searchTotal > 0) return;
  const el = scrollRef.current;
  if (!el) return;
  // Anclar al fondo del scroll range — el último mensaje del
  // array es el que acaba de llegar (o el cargado al cambiar de
  // tab), y queda visible.
  el.scrollTop = el.scrollHeight;
  // Doble rAF como belt-and-suspenders para el camino async de
  // loadSession: si el contenido se monta un par de frames
  // después del primer scrollTop, reiteramos para garantizar el
  // ancla.
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const el2 = scrollRef.current;
      if (el2) el2.scrollTop = el2.scrollHeight;
    });
  });
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, [activeId, messages.length, lastMsgSig, lastMsgIsStreaming, searchTotal]);

  // EP-0028 (deprecated): the auto-scroll-on-keyboard-up behaviour
  // used to live here, driven by `useMobileKeyboard().keyboardHeight`.
// That hook was removed (it leaked stuck values when the keyboard
// closed on several Android browsers, leaving a huge empty gap below
// the chat input). We now rely on CSS `100dvh` + the sticky
// `<textarea>` for the keyboard interaction; the transcript keeps
// its last scroll position. Reintroduce an equivalent here only if
// we find users complaining about messages being clipped behind the
// soft keyboard on real devices.

  const handleSend = useCallback(async () => {
    const text = input.trim();
    if (!text || isStreaming || !activeTab || !activeTab.sessionId) return;
    setInput("");
    setError(null);

    const sessionId = activeTab.sessionId;
    const tabId = activeTab.id;

    // ─── Order fix: user message BEFORE assistant ─────────────────────
    // We add the user message NOW with a NEGATIVE temp id (a
    // sentinel that never collides with the daemon's positive
    // autoincrement ids). The WS handler in `useChatTabs` replaces
    // this temp id with the canonical one when MessageAppended
    // arrives — keeping the row at its current position in the
    // array, so the chat reads "user → assistant" top-to-bottom.
    //
    // Earlier the assistant shell was pre-created here BEFORE the
    // user message, which is what made the chat render "assistant
    // thinking → user message" — and F5 (which reloads from the
    // DB in chronological order) was the only way to see the right
    // order. With this fix, the assistant shell is now created
    // lazily by `applyStreamChunk` on the first SSE chunk, AFTER
    // the user message is in the array.
    const tempUserId = -Date.now();
    const userMsg: Message = {
      id: tempUserId,
      session_id: sessionId,
      role: "user",
      content: text,
      ts: new Date().toISOString(),
    };
    updateTab(tabId, (prev) => ({
      ...prev,
      messages: [...prev.messages, userMsg],
    }));

    const t0 = Date.now();
    try {
      // `sendStream` del hook dispara el turno en el backend y resuelve
      // cuando el stream SSE termina. Los chunks NO se aplican aquí:
      // llegan por el WS `/v1/events` (fuente única) y se aplican en
      // `useChatTabs`, ordenados por el `seq` monotónico del daemon.
      // Esto elimina la doble aplicación SSE+WS que causaba la
      // duplicación/truncación en el receptor.
      await sendStream(text);
      const durationMs = Date.now() - t0;
      // Apply metrics to the assistant message we just streamed. We
      // look up by ROLE (last assistant) from `tabsRef` rather than
      // by `streamingMessageId` because the WS MessageAppended
      // handler replaces the shell id with the canonical one as the
      // stream finishes — the id we tracked is now stale.
      const liveTab = tabsRef.current.find((t) => t.id === tabId);
      const lastAssistant = liveTab
        ? [...liveTab.messages].reverse().find((m) => m.role === "assistant")
        : null;
      if (lastAssistant) {
        const totalText = (lastAssistant.content ?? "") + (lastAssistant.thinking ?? "");
        if (lastAssistant.content) {
          setStreamingMetrics(tabId, lastAssistant.id, computeMetrics(durationMs, totalText));
        }
      }
    } catch (e) {
      setError((e as Error).message);
      // EP-2026-08-31: stream failure recovery. Drop the dead
      // sessionId so the auto-create effect below spins up a fresh
      // one for the same tab. The user can re-send without
      // manually closing the tab.
      updateTab(tabId, () => ({ sessionId: undefined }));
    } finally {
      // EP-0028: refresh the tab title after the first user
      // message — the backend has populated `sessions.summary` with
      // a preview of the message.
      if (activeTab.sessionId) {
        void refreshTabSummary(activeTab.sessionId, activeTab.id);
      }
      inputRef.current?.focus();
    }
  }, [input, isStreaming, activeTab, sendStream, updateTab, refreshTabSummary, setStreamingMetrics]);

  const handleCancel = useCallback(async () => {
    if (!activeTab?.sessionId) return;
    // Dos cortes en paralelo: cliente (AbortController vía hook) +
    // servidor (cancelSession endpoint). Mismas semantics que antes.
    cancelStream();
    try {
      await cancelSession(activeTab.sessionId);
    } catch (e) {
      setError((e as Error).message);
    }
  }, [activeTab?.sessionId, cancelStream]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Send only on Shift+Enter or Ctrl+Enter. Bare Enter inserts a
    // newline (the textarea's default behavior). This matches the
    // convention used by Slack, Discord and most modern chat UIs.
    if (e.key === "Enter" && (e.shiftKey || e.ctrlKey)) {
      e.preventDefault();
      void handleSend();
    }
  }, [handleSend]);

  // EP-0028 HMR-fix: `messages`, `sessionId`, and `sessionModel` were
  // moved to the top of the component (right after the refs) so they
  // are in scope when the `searchTotal` useMemo runs. Keeping a
  return (
    <div className="chat-layout" ref={fullscreenContainerRef}>
      <ChatHeader
        tabs={tabs}
        activeId={activeId}
        activeTab={activeTab}
        sessionId={sessionId}
        onSelectTab={selectTab}
        onCloseTab={closeTab}
        onCreateTab={createTab}
        onRenameTab={(id, title) => {
          renameTab(id, title).catch((e) =>
            setError(`Rename failed: ${(e as Error).message}`),
          );
        }}
        searchQuery={searchQuery}
        searchActive={searchActive}
        searchTotal={searchTotal}
        onSearchQueryChange={handleSearchQueryChange}
        onSearchActiveChange={handleSearchActiveChange}
        onSearchClear={handleSearchClear}
      />

      <ChatMain
        error={error}
        messages={messages}
        isStreaming={isStreaming}
        streamingMessageId={streamingMessageId}
        emptyAgent={inProcessDefaultAgent}
        sessionId={sessionId}
        scrollRef={scrollRef}
        showHistory={showHistory}
        onLoadSession={loadSession}
        onSearchActiveChange={handleSearchActiveChange}
        searchQuery={searchQuery}
      />

      <ChatFooter
        sessionId={sessionId}
        sessionModel={sessionModel}
        onChangeModel={(m) => activeTab && updateTab(activeTab.id, { sessionModel: m })}
        currentAgent={activeTab?.sessionAgent ?? daemonDefaultAgentId ?? ""}
        onChangeAgent={(id) => {
          if (!activeTab) return;
          if (activeTab.sessionAgent !== id) {
            updateTab(activeTab.id, { sessionAgent: id });
          }
        }}
        showHistory={showHistory}
        onToggleHistory={() => setShowHistory(!showHistory)}
        voiceOverlayOpen={voiceOverlayOpen}
        onOpenVoiceCall={() => setVoiceOverlayOpen(true)}
        isFullscreen={isFullscreen}
        onToggleFullscreen={handleToggleFullscreen}
        defaultAgent={defaultAgent}
        input={input}
        onInputChange={setInput}
        onKeyDown={handleKeyDown}
        onMicTranscript={setInput}
        isStreaming={isStreaming}
        onSend={() => void handleSend()}
        onCancel={() => void handleCancel()}
        inputRef={inputRef}
      />

      {/* EP-0002: voice call overlay (fullscreen, hands-free mode) */}
      <PanelToggle id="chat-voice-overlay">
        <VoiceCallOverlay
          open={voiceOverlayOpen}
          sessionId={sessionId}
          onClose={() => setVoiceOverlayOpen(false)}
        />
      </PanelToggle>
    </div>
  );
}
