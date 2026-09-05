// useChatTabs — manages chat tabs synced from the daemon per user.
//
// Source of truth: `/v1/sessions` filtered server-side by the
// authenticated `user_id`. Tabs are shared across every device
// logged in as the same user; `/v1/events` WS pushes realtime
// updates (SessionStarted/Ended) so a tab opened on the laptop shows
// up on the mobile seconds later, and vice versa.
//
// What stays LOCAL (per-tab UI state, not persisted on the server):
//   - `title` and `manuallyRenamed` flag
//   - `searchQuery` / `searchActive` (in-chat search)
//   - `messages` (the full transcript including the streaming timeline)
//   - `sessionModel` / `sessionAgent` (overrides the user has applied)
//
// Local state is keyed by `sessionId` once it exists. While a tab
// has no session yet (right after the user clicks `+`), it lives in
// a local-only slot and gets cleaned up if the session-creation
// fails.

import { useCallback, useEffect, useRef, useState } from "react";
import type { Message, MessageMetrics } from "../types";
import type { ModelSelection } from "../components/ModelSelector";
import { apiGet, apiPost, getToken } from "../api/client";
import { getWebClientId } from "../shared/clientId";
import {
  applyStreamChunk as applyChunkToMessage,
  initAccumulator,
  type StreamAccumulator,
} from "../components/chat/streaming/applyChunk";
import { parseStreamChunk, type StreamChunk } from "../components/chat/streaming/chunk";

export interface ChatTab {
  id: string;
  title: string;
  sessionId: string | null;
  sessionModel: ModelSelection | null;
  sessionAgent: string;
  messages: Message[];
  createdAt: number;
  manuallyRenamed: boolean;
  searchQuery: string;
  searchActive: number;
}

const NO_AGENT_SENTINEL = "";

function uuid(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return "tab-" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

/** Subset of SessionSummary that drives the tab UI. */
interface SessionRow {
  session_id: string;
  agent_id: string;
  provider_id?: string | null;
  model?: string | null;
  started_at: string;
  ended_at?: string | null;
  summary?: string | null;
}

export function useChatTabs(defaultAgentId: string | null = null) {
  const [tabs, setTabs] = useState<ChatTab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  // Track session_ids we've already created a local tab for, to dedupe
  // SessionStarted events that fire right after createSession.
  const seenSessions = useRef<Set<string>>(new Set());
  // Mirror `tabs` and `activeId` into refs so the long-lived WS
  // `onmessage` handler (which captures stale values inside the
  // useEffect closure) always reads the latest state. Without this,
  // chunks arriving while the user opens a new tab can be routed to
  // a tab that no longer exists in `tabs.find()`.
  const tabsRef = useRef<ChatTab[]>(tabs);
  tabsRef.current = tabs;
  const activeIdRef = useRef<string | null>(activeId);
  activeIdRef.current = activeId;
  // Monotonic counter for client-side assistant shell ids. Strictly
  // positive and strictly increasing, so two rapid shell creations
  // (e.g. SSE + WS paths racing on cross-device) never collide, and
  // the shell id is distinguishable from the user message's negative
  // temp id (`-Date.now()` set by ChatPanel.handleSend).
  const shellIdCounterRef = useRef<number>(0);
  // Per-session transient StreamAccumulator. The reducer needs
  // `lastNonContentEvent` / `lastToolName` across consecutive chunks to
  // detect the wire-format quirk where the backend embeds a tool's
  // stdout in a `content` chunk. The daemon only allows one active
  // stream per session, so a Map<sessionId, …> is sufficient.
  const streamAccumulatorsRef = useRef<Map<string, StreamAccumulator>>(
    new Map(),
  );
  // Per-session deduper: defends against the daemon emitting the same
  // chunk in a loop (root cause fixed separately). Reset on
  // MessageAppended(assistant) along with the accumulator.
  // Per-session last applied `seq` (PARTE A/B). The daemon tags every
  // stream chunk with a strictly-increasing per-session `seq`. We apply
  // a chunk ONLY if `chunk.seq > lastSeq[sessionId]`, then bump
  // `lastSeq`. This makes the `/v1/events` WS the single source of
  // truth: even though a chunk may reach us more than once (e.g. a
  // reconnect replays, or StrictMode double-mounts a socket), it is
  // applied exactly once and in order. Chunks without a `seq` (legacy)
  // or with `seq <= lastSeq` are ignored. Reset on stream boundaries
  // (see `clearSessionStreamState` + first chunk of a new turn).
  const lastSeqRef = useRef<Map<string, number>>(new Map());
  // The id of the assistant message currently being streamed per
  // session. Populated on the first chunk; the caller (ChatPanel)
  // reads it back to know which row should display the streaming
  // caret. Cleared on stream end (MessageAppended assistant role).
  const streamMessageIdsRef = useRef<Map<string, number>>(new Map());

  // Helper: create or update the local tab for a given backend session.
  const upsertSession = useCallback((row: SessionRow, opts?: { activate?: boolean }) => {
    setTabs((prev) => {
      if (prev.some((t) => t.sessionId === row.session_id)) return prev;
      const fresh: ChatTab = {
        id: uuid(),
        title:
          row.summary && row.summary.trim().length > 0
            ? row.summary
            : `Chat ${prev.length + 1}`,
        sessionId: row.session_id,
        sessionModel:
          row.provider_id && row.model
            ? { provider_id: row.provider_id, model: row.model }
            : null,
        sessionAgent: row.agent_id,
        messages: [],
        createdAt: Date.parse(row.started_at) || Date.now(),
        manuallyRenamed: !!row.summary,
        searchQuery: "",
        searchActive: 0,
      };
      const next = [...prev, fresh];
      if (opts?.activate) setActiveId(fresh.id);
      return next;
    });
  }, []);

  // One-time hydration: fetch the active sessions for this user, build
  // local tabs for each. The daemon is the source of truth — there's
  // no localStorage fallback (every device re-syncs from the server
  // on mount).
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await apiGet<{ sessions: SessionRow[] }>("/v1/sessions");
        if (cancelled) return;
        res.sessions.forEach((s) => seenSessions.current.add(s.session_id));
        setTabs(
          res.sessions.map((row, idx) => ({
            id: uuid(),
            title:
              row.summary && row.summary.trim().length > 0
                ? row.summary
                : `Chat ${idx + 1}`,
            sessionId: row.session_id,
            sessionModel:
              row.provider_id && row.model
                ? { provider_id: row.provider_id, model: row.model }
                : null,
            sessionAgent: row.agent_id,
            messages: [],
            createdAt: Date.parse(row.started_at) || Date.now(),
            manuallyRenamed: !!row.summary,
            searchQuery: "",
            searchActive: 0,
          })),
        );
      } catch (e) {
        console.warn("useChatTabs: failed to load sessions", e);
      } finally {
        if (!cancelled) setLoaded(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // WS subscription: every WS message is already filtered server-side
  // by the calling user's `user_id` (see daemon/core/src/router/ws.rs).
  // We just react to the events we care about.
  //
  // Stream chunks (content / thinking / tool_call / tool_result) arrive
  // ONLY via this WS — it is the single source of chunks for BOTH the
  // sender and every receiver (PARTE B). Each chunk carries a monotonic
  // per-session `seq` (PARTE A); `applyStreamChunk` applies it exactly
  // once and in order (see the `seq > lastSeq` guard there). There is
  // no longer a local-SSE skip, text deduper, or zombie-socket guard —
  // the `seq` ordering makes re-delivery idempotent.
  //
  // MessageAppended (user role) does the local-optimistic dedup: if
  // ChatPanel added the user message locally with a negative temp id,
  // this handler replaces its id with the canonical daemon id and
  // keeps the message at its position (so the order is preserved).
  //
  // MessageAppended (assistant role) is the "stream done" marker —
  // it clears the per-session accumulator + last-seq so a future
  // stream starts fresh.
  useEffect(() => {
    const token = getToken();
    if (!token) return;
    const url = buildWsUrl("/v1/events", token);
    const ws = new WebSocket(url);
    ws.onmessage = (e) => {
      let evt: Record<string, unknown>;
      try {
        evt = JSON.parse(e.data);
      } catch {
        return;
      }
      const type = evt.type;
      const sid = evt.session_id as string | undefined;
      if (type === "session_started" && sid) {
        if (seenSessions.current.has(sid)) return;
        seenSessions.current.add(sid);
        upsertSession({
          session_id: sid,
          agent_id: "",
          started_at: new Date().toISOString(),
          summary: null,
        });
        void apiGet<{ sessions: SessionRow[] }>("/v1/sessions")
          .then((res) => {
            const row = res.sessions.find((s) => s.session_id === sid);
            if (row) upsertSession(row);
          })
          .catch(() => {});
      } else if (type === "session_ended" && sid) {
        seenSessions.current.delete(sid);
        clearSessionStreamState(sid);
        setTabs((prev) => {
          const next = prev.filter((t) => t.sessionId !== sid);
          // Use activeIdRef (not the captured `activeId`): if a tab
          // switch raced with this event, the closure copy may be
          // stale and we'd kick the user out of the tab they just
          // focused.
          const cur = activeIdRef.current;
          if (cur && !next.some((t) => t.id === cur) && next.length > 0) {
            setActiveId(next[0]!.id);
          } else if (next.length === 0) {
            setActiveId(null);
          }
          return next;
        });
      } else if (type === "message_appended" && sid) {
        // Other device (or this one on reload) added a message to a
        // session. Mirror it into the local tab's transcript with
        // DEDUP against any local-optimistic copy:
        //   - User message: ChatPanel.handleSend adds it with a negative
        //     temp id before SSE opens; replace that temp id with the
        //     canonical one (keeps the row at its position so chat
        //     order stays "user, assistant").
        //   - Assistant message: the SSE path created a local shell
        //     with a positive id from `shellIdCounterRef`. REPLACE
        //     that shell's id with the canonical one — without this
        //     we'd append the canonical row AFTER the shell, leaving
        //     the user with two assistant rows (a truncated live
        //     preview + a full duplicate).
        const message_id = evt.message_id as number | undefined;
        const role = (evt.role as string) ?? "user";
        const content = (evt.content as string) ?? "";
        const ts = (evt.ts as string) ?? new Date().toISOString();
        const thinking = evt.thinking as string | undefined;
        if (message_id == null) return;
        // Capture the local shell id BEFORE we clear stream state.
        // `streamMessageIdsRef` is wiped by `clearTurnStreamState`
        // for the assistant role below, so the lookup has to happen
        // first.
        const localShellId =
          role === "assistant"
            ? (streamMessageIdsRef.current.get(sid) ?? null)
            : null;
        if (role === "assistant") {
          // Stream-done marker: clear per-TURN state (accumulator +
          // shell tracking) but KEEP `lastSeq` — the daemon's seq is
          // monotonic for the whole session, so the watermark must
          // survive across turns to reject late/re-delivered chunks.
          clearTurnStreamState(sid);
        }
        setTabs((prev) => {
          const tab = prev.find((t) => t.sessionId === sid);
          if (!tab) return prev;
          // Already have the canonical row → no-op.
          if (tab.messages.some((m) => m.id === message_id)) return prev;

          // Assistant + we streamed locally → REPLACE the shell's id
          // with the canonical one. Keep the accumulated content (the
          // backend's content is the same — `content` here is the
          // ground truth, so overwrite in case the shell missed late
          // chunks due to a race).
          if (role === "assistant" && localShellId != null) {
            const shellIdx = tab.messages.findIndex(
              (m) => m.id === localShellId,
            );
            if (shellIdx >= 0) {
              const updated = [...tab.messages];
              const existing = updated[shellIdx]!;
              updated[shellIdx] = {
                ...existing,
                id: message_id,
                ts,
                content,
                thinking: thinking ?? existing.thinking,
              };
              return prev.map((t) =>
                t.id === tab.id ? { ...t, messages: updated } : t,
              );
            }
          }

          // User role + local optimistic with same content → replace
          // the temp id with the canonical one (keeps the row at its
          // current position so chat order stays "user, assistant").
          if (role === "user") {
            const localIdx = tab.messages.findIndex(
              (m) =>
                m.role === "user" &&
                m.id < 0 &&
                m.content === content,
            );
            if (localIdx >= 0) {
              const updated = [...tab.messages];
              const existing = updated[localIdx]!;
              updated[localIdx] = {
                ...existing,
                id: message_id,
                ts,
                thinking: thinking ?? existing.thinking,
              };
              return prev.map((t) =>
                t.id === tab.id ? { ...t, messages: updated } : t,
              );
            }
          }

          // No dedup target — append (other device, or page
          // reloaded mid-send and there's no local optimistic).
          return prev.map((t) =>
            t.id === tab.id
              ? {
                  ...t,
                  messages: [
                    ...tab.messages,
                    {
                      id: message_id,
                      session_id: sid,
                      role: role as Message["role"],
                      content,
                      thinking: thinking ?? undefined,
                      ts,
                    },
                  ],
                }
              : t,
          );
        });
      } else if (
        (type === "content" ||
          type === "thinking" ||
          type === "tool_call" ||
          type === "tool_result") &&
        sid
      ) {
        // Single source of chunks. This WS is the ONLY path that
        // applies stream chunks — for the sender AND every receiver.
        // The daemon-assigned `seq` (parsed below and enforced in
        // `applyStreamChunk`) makes re-delivery idempotent, so no
        // local-SSE skip is needed anymore.
        const chunk = parseStreamChunk(evt);
        if (!chunk) return;
        // Route to the tab whose sessionId matches. Read from the
        // ref so chunks that arrive while tabs are being mutated
        // (e.g. createTab mid-stream) don't lose the new tab.
        const tab = tabsRef.current.find((t) => t.sessionId === sid);
        if (tab) applyStreamChunk(tab.id, sid, chunk);
      }
    };
    return () => {
      ws.close();
    };
    // The socket must live for the WHOLE component lifetime — a single
    // long-lived subscription to `/v1/events`. Everything the handler
    // needs is read from refs (`tabsRef`, `activeIdRef`, `seenSessions`,
    // `streamMessageIdsRef`, `lastSeqRef`) or from stable
    // `useCallback`s (`upsertSession`, `applyStreamChunk`,
    // `clearSessionStreamState`), so an empty dep array is correct.
    //
    // BUGFIX (cross-device realtime): the dep array was `[activeId]`,
    // which tore down and re-opened the socket on EVERY tab switch.
    // During each reconnect there was a blind window where events from
    // another device (the user's own prompt + the assistant's stream)
    // were dropped — the receiving device only caught up on the next
    // full reload (F5) via `/v1/sessions` + `getSessionMessages`. A
    // single persistent socket removes that window.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // First-time UX: if the user has no active sessions yet, auto-create
  // an empty "draft" tab so the chat panel isn't empty. The first
  // message they send will create the actual session.
  useEffect(() => {
    if (loaded && tabs.length === 0 && activeId === null) {
      const fresh: ChatTab = {
        id: uuid(),
        title: "Chat 1",
        sessionId: null,
        sessionModel: null,
        sessionAgent: defaultAgentId ?? NO_AGENT_SENTINEL,
        messages: [],
        createdAt: Date.now(),
        manuallyRenamed: false,
        searchQuery: "",
        searchActive: 0,
      };
      setTabs([fresh]);
      setActiveId(fresh.id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loaded]);

  // Neighbor selection if active tab got removed (e.g. SessionEnded
  // for the tab in focus).
  useEffect(() => {
    if (activeId === null && tabs.length > 0) {
      setActiveId(tabs[0]!.id);
    }
  }, [activeId, tabs]);

  // Back-fill the agent id on draft tabs once the daemon reports it.
  useEffect(() => {
    if (!defaultAgentId) return;
    setTabs((prev) => {
      const needsFill = prev.some((t) => t.sessionAgent === NO_AGENT_SENTINEL);
      if (!needsFill) return prev;
      return prev.map((t) =>
        t.sessionAgent === NO_AGENT_SENTINEL
          ? { ...t, sessionAgent: defaultAgentId }
          : t,
      );
    });
  }, [defaultAgentId]);

  const activeTab = tabs.find((t) => t.id === activeId) ?? null;

  const createTab = useCallback(async () => {
    // Server-side: POST creates the session, returns session_id. The
    // WS broadcast will fire SessionStarted, but we also optimistically
    // add the tab here so the UI moves instantly without waiting for
    // the round-trip.
    const id = uuid();
    const tab: ChatTab = {
      id,
      title: `Chat ${tabs.length + 1}`,
      sessionId: null,
      sessionModel: null,
      sessionAgent: defaultAgentId ?? NO_AGENT_SENTINEL,
      messages: [],
      createdAt: Date.now(),
      manuallyRenamed: false,
      searchQuery: "",
      searchActive: 0,
    };
    setTabs((prev) => [...prev, tab]);
    setActiveId(id);

    try {
      const res = await apiPost<{
        session_id: string;
        agent_id: string;
      }>("/v1/sessions", {
        agent_id: defaultAgentId ?? "default",
        client_id: getWebClientId(),
      });
      seenSessions.current.add(res.session_id);
      setTabs((prev) =>
        prev.map((t) =>
          t.id === id ? { ...t, sessionId: res.session_id, sessionAgent: res.agent_id } : t,
        ),
      );
    } catch (e) {
      console.error("useChatTabs: failed to create session", e);
    }
    return tab;
  }, [tabs.length, defaultAgentId]);

  /**
   * Load (or focus) a tab for a given backend session. Used by the
   * History sidebar (closed sessions) — the user clicks a past
   * conversation, we reactivate it server-side and focus the tab.
   */
  const createTabFromSession = useCallback(
    async (sessionId: string, summary: string | null) => {
      // Existing tab for this session? Just focus.
      const existing = tabs.find((t) => t.sessionId === sessionId);
      if (existing) {
        setActiveId(existing.id);
        return existing;
      }
      // Otherwise: reactivate the session (sets ended_at = NULL) and
      // create a local tab. If activation fails (e.g. session doesn't
      // exist), fall back to creating a fresh one.
      try {
        await apiPost(`/v1/sessions/${sessionId}/reactivate`);
      } catch {
        /* swallow — we'll still create the tab; the user can retry */
      }
      const id = uuid();
      const tab: ChatTab = {
        id,
        title: summary && summary.trim().length > 0 ? summary : "Chat",
        sessionId,
        sessionModel: null,
        sessionAgent: defaultAgentId ?? NO_AGENT_SENTINEL,
        messages: [],
        createdAt: Date.now(),
        manuallyRenamed: true,
        searchQuery: "",
        searchActive: 0,
      };
      seenSessions.current.add(sessionId);
      setTabs((prev) => [...prev, tab]);
      setActiveId(id);
      return tab;
    },
    [tabs, defaultAgentId],
  );

  const closeTab = useCallback(async (id: string) => {
    // Find the session_id of the tab we're closing. If it has one,
    // tell the daemon to deactivate it (set ended_at). The WS event
    // will also remove the tab from local state — we dedupe by
    // matching session_id.
    const tab = tabs.find((t) => t.id === id);
    if (tab?.sessionId) {
      try {
        await apiPost(`/v1/sessions/${tab.sessionId}/cancel`);
      } catch (e) {
        console.error("useChatTabs: cancel failed", e);
      }
      seenSessions.current.delete(tab.sessionId);
    }
    setTabs((prev) => {
      const next = prev.filter((t) => t.id !== id);
      if (next.length === 0) {
        // Keep at least one (draft) tab so the UI doesn't collapse.
        const fresh: ChatTab = {
          id: uuid(),
          title: "Chat 1",
          sessionId: null,
          sessionModel: null,
          sessionAgent: defaultAgentId ?? NO_AGENT_SENTINEL,
          messages: [],
          createdAt: Date.now(),
          manuallyRenamed: false,
          searchQuery: "",
          searchActive: 0,
        };
        setActiveId(fresh.id);
        return [fresh];
      }
      if (id === activeId) {
        const idx = prev.findIndex((t) => t.id === id);
        const neighbor = next[Math.min(idx, next.length - 1)] ?? next[0]!;
        setActiveId(neighbor.id);
      }
      return next;
    });
  }, [tabs, activeId, defaultAgentId]);

  const closeSession = useCallback(async (sessionId: string) => {
    // Cancel the session server-side (sets ended_at). WS event will
    // remove the matching local tab.
    try {
      await apiPost(`/v1/sessions/${sessionId}/cancel`);
    } catch (e) {
      console.error("useChatTabs: closeSession failed", e);
    }
    seenSessions.current.delete(sessionId);
  }, []);

  const selectTab = useCallback((id: string) => {
    setActiveId(id);
  }, []);

  /**
   * Rename a tab. Persists to the backend so the History panel shows
   * the same title.
   */
  const renameTab = useCallback(
    async (id: string, title: string): Promise<void> => {
      const trimmed = title.trim();
      if (!trimmed) return;

      let sessionId: string | null = null;
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== id) return t;
          sessionId = t.sessionId;
          return { ...t, title: trimmed, manuallyRenamed: true };
        }),
      );

      if (sessionId) {
        try {
          const { renameSession } = await import("../api/sessions");
          await renameSession(sessionId, trimmed);
        } catch (e) {
          throw e;
        }
      }
    },
    [],
  );

  /**
   * Apply the backend's auto-generated summary to the tab, but ONLY
   * if the user hasn't manually renamed it.
   */
  const updateTabSummary = useCallback((id: string, summary: string) => {
    const trimmed = summary.trim();
    if (!trimmed) return;
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== id) return t;
        if (t.manuallyRenamed) return t;
        return { ...t, title: trimmed };
      }),
    );
  }, []);

  /**
   * Apply a partial patch to the tab with the given id. See the
   * streaming-chunk usage in ChatPanel for why the function form
   * matters.
   */
  const updateTab = useCallback(
    (id: string, patch: Partial<ChatTab> | ((prev: ChatTab) => Partial<ChatTab>)) => {
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== id) return t;
          const p = typeof patch === "function" ? patch(t) : patch;
          return { ...t, ...p };
        }),
      );
    },
    [],
  );

  const updateSessionSummary = useCallback(async () => {
    // Force-refresh the tab list from the daemon (e.g. after the
    // first user message of a fresh session so the title picks up
    // the backend's auto-summary).
    try {
      const res = await apiGet<{ sessions: SessionRow[] }>("/v1/sessions");
      setTabs((prev) => {
        const byId = new Map(res.sessions.map((s) => [s.session_id, s]));
        return prev.map((t) => {
          if (!t.sessionId) return t;
          const row = byId.get(t.sessionId);
          if (!row) return t;
          return {
            ...t,
            title: t.manuallyRenamed ? t.title : (row.summary ?? t.title),
            sessionModel:
              row.provider_id && row.model
                ? { provider_id: row.provider_id, model: row.model }
                : t.sessionModel,
          };
        });
      });
    } catch (e) {
      console.error("useChatTabs: refresh failed", e);
    }
  }, []);

  // ─── Single-source-of-truth chunk apply ────────────────────────────────
  //
  // ALL stream chunks flow through here from the `/v1/events` WS — for
  // the sender AND every receiver (PARTE B). The local SSE path no
  // longer applies chunks (it only triggers the turn on the backend).
  //
  // Ordering is enforced by the daemon-assigned monotonic per-session
  // `seq` (PARTE A):
  //
  //   1. Ignore chunks with no `seq` (legacy) or `seq <= lastSeq` —
  //      already applied, or a re-delivery from a reconnect / duplicate
  //      socket. This is what makes the WS the single, idempotent
  //      source of truth.
  //   2. Otherwise apply and bump `lastSeq[sessionId]`.
  //   3. Find the assistant message being streamed for this session
  //      (tracked via `streamMessageIdsRef`; falls back to the LAST
  //      assistant message in the tab — typical for reload-mid-stream).
  //      If none exists, create one.
  //   4. Apply via `applyStreamChunk` (the pure reducer utility —
  //      collapses timeline entries, redirects tool_output-as-content,
  //      surfaces errors).
  //   5. Write the message back to the tab and persist the new
  //      accumulator so the NEXT chunk has the right
  //      `lastNonContentEvent` / `lastToolName`.
  //
  // Returns `{ messageId, error }` so the caller can update UI state
  // (caret, error banner) without re-deriving it from the messages
  // array.
  const applyStreamChunk = useCallback(
    (
      tabId: string,
      sessionId: string,
      chunk: StreamChunk,
    ): { messageId: number | null; error: string | null } => {
      // Order gate. Two classes of chunk:
      //
      //   A) SEQ-CARRYING stream chunks (content / thinking /
      //      tool_call / tool_result). The daemon stamps a monotonic
      //      per-session `seq` on every one of these. Apply only if
      //      `seq > lastSeq[sessionId]`; a chunk with `seq <= lastSeq`
      //      is an already-applied / re-delivered copy and is dropped.
      //      A seq-carrying chunk that arrives WITHOUT a seq is a
      //      legacy/anomalous frame and is dropped too (the backend
      //      always stamps these).
      //
      //   B) NON-ORDERED chunks (approval_request / approval_resolved
      //      / error). These never carry a seq and are not part of the
      //      text stream. Always apply — the reducer is idempotent for
      //      them (approvals match by `id`, error just sets a banner),
      //      so a duplicate WS delivery is harmless.
      const isOrdered =
        chunk.type === "content" ||
        chunk.type === "thinking" ||
        chunk.type === "tool_call" ||
        chunk.type === "tool_result";
      if (isOrdered) {
        const seq =
          "seq" in chunk && typeof chunk.seq === "number"
            ? chunk.seq
            : undefined;
        if (seq === undefined) return { messageId: null, error: null };
        const lastSeq = lastSeqRef.current.get(sessionId) ?? -Infinity;
        if (seq <= lastSeq) return { messageId: null, error: null };
        lastSeqRef.current.set(sessionId, seq);
      }

      const acc = streamAccumulatorsRef.current.get(sessionId) ?? initAccumulator();

      // Error chunks carry no assistant text — surface the message
      // directly. Doing it here (outside the setTabs updater) makes the
      // returned `error` reliable: reading a value mutated INSIDE the
      // React state updater is racy because the updater may run
      // deferred/batched, so the function would return before it ran.
      if (chunk.type === "error") {
        return { messageId: null, error: chunk.message };
      }

      let resultMessageId: number | null = null;
      let resultError: string | null = null;

      setTabs((prev) => {
        const tab = prev.find((t) => t.id === tabId);
        if (!tab) return prev;

        // Find the assistant message we're streaming into:
        //   1. The id tracked in `streamMessageIdsRef` (set on the
        //      first chunk of this stream).
        //   2. Else the last assistant message in the tab (typical
        //      for reload-mid-stream — the historical assistant is
        //      "the streaming one" until a new one is created).
        const trackedId = streamMessageIdsRef.current.get(sessionId);
        let target = trackedId != null
          ? tab.messages.find((m) => m.id === trackedId && m.role === "assistant")
          : undefined;
        if (!target) {
          // Fallback: latch onto the last assistant message — but
          // ONLY when it is the very last row in the transcript. That
          // is the reload-mid-stream case: the historical assistant
          // (loaded from the DB) IS the one still streaming until a
          // new turn creates a fresh shell.
          //
          // BUGFIX (timeline out-of-order): on the FIRST chunk of a
          // NEW turn, `streamMessageIdsRef` was already cleared by the
          // previous turn's `message_appended(assistant)`, so
          // `trackedId` is undefined. The user message the sender just
          // added optimistically is now the LAST row — so the "last
          // row is assistant" guard is false and we correctly fall
          // through to "create a fresh shell" below, appending it
          // AFTER the new user message (send-order preserved). Without
          // this guard the fallback would latch onto the PREVIOUS
          // turn's reply and strand the new user message at the bottom.
          const lastMsg = tab.messages[tab.messages.length - 1];
          if (lastMsg && lastMsg.role === "assistant") {
            target = lastMsg;
            streamMessageIdsRef.current.set(sessionId, target.id);
          }
        }

        if (!target) {
          // No assistant message yet — create one. This is the
          // CROSS-DEVICE case (or local SSE if ChatPanel didn't pre-
          // create a shell, which is the new design). The shell is
          // appended AFTER the user message so the timeline reads
          // top-to-bottom in send-order.
          //
          // The id is strictly positive (so it stays distinguishable
          // from the user message's `-Date.now()` temp id) and
          // strictly increasing via `shellIdCounterRef` — a previous
          // version used `Date.now() + Math.random()` which could
          // collide when two shells were created in the same ms (e.g.
          // SSE and WS paths racing on cross-device).
          const newId = ++shellIdCounterRef.current;
          const shell: Message = {
            id: newId,
            session_id: sessionId,
            role: "assistant",
            content: "",
            ts: new Date().toISOString(),
            timeline: [],
          };
          const r = applyChunkToMessage(shell, chunk, acc);
          streamAccumulatorsRef.current.set(sessionId, r.acc);
          streamMessageIdsRef.current.set(sessionId, newId);
          resultMessageId = newId;
          resultError = r.error;
          return prev.map((t) =>
            t.id === tabId
              ? { ...tab, messages: [...tab.messages, r.message] }
              : t,
          );
        }

        // Existing assistant message — apply the chunk to its
        // current state. This handles BOTH the local SSE path
        // (ChatPanel created the shell ahead of time) and the
        // remote WS path (we found an existing shell from another
        // device's stream).
        const r = applyChunkToMessage(target, chunk, acc);
        streamAccumulatorsRef.current.set(sessionId, r.acc);
        streamMessageIdsRef.current.set(sessionId, target.id);
        resultMessageId = target.id;
        resultError = r.error;
        return prev.map((t) =>
          t.id === tabId
            ? {
                ...tab,
                messages: tab.messages.map((m) => (m.id === target!.id ? r.message : m)),
              }
            : t,
        );
      });

      return { messageId: resultMessageId, error: resultError };
    },
    [],
  );

  // Read-only accessor for ChatPanel: returns the assistant message
  // id currently being streamed for the given tab (or null).
  const getStreamingMessageId = useCallback(
    (tabId: string): number | null => {
      for (const t of tabs) {
        if (t.id === tabId && t.sessionId) {
          return streamMessageIdsRef.current.get(t.sessionId) ?? null;
        }
      }
      return null;
    },
    [tabs],
  );

  // Set metrics on the (now-completed) streaming message. Called by
  // ChatPanel after `sendStream()` resolves.
  const setStreamingMetrics = useCallback(
    (tabId: string, messageId: number, metrics: MessageMetrics) => {
      setTabs((prev) =>
        prev.map((t) =>
          t.id !== tabId
            ? t
            : {
                ...t,
                messages: t.messages.map((m) =>
                  m.id === messageId ? { ...m, metrics } : m,
                ),
              },
        ),
      );
    },
    [],
  );

  // Clear per-TURN streaming state. Called on MessageAppended
  // (assistant role) — the "stream done" marker. Resets the accumulator
  // and the tracked assistant-shell id so the NEXT turn starts a fresh
  // message.
  //
  // BUGFIX (residual duplication): this does NOT clear `lastSeqRef`.
  // The daemon's `seq` is monotonic per SESSION (it never restarts per
  // turn — see EventsLayer::next_seq), so `lastSeq` must also persist
  // for the whole session. Clearing it per turn re-opened the door for
  // a late/re-delivered chunk from the finished turn (whose seq we had
  // already seen) to be re-applied on the next turn — the exact
  // duplication we're eliminating. `lastSeq` is only reset when the
  // SESSION ends (`clearSessionStreamState`).
  const clearTurnStreamState = useCallback((sessionId: string) => {
    streamAccumulatorsRef.current.delete(sessionId);
    streamMessageIdsRef.current.delete(sessionId);
  }, []);

  // Clear ALL per-session streaming state, including the monotonic
  // `lastSeq` watermark. Only safe when the SESSION ends (session_ended)
  // — at that point the daemon's per-session seq counter is also gone,
  // so a brand-new session legitimately restarts at seq 1.
  const clearSessionStreamState = useCallback((sessionId: string) => {
    streamAccumulatorsRef.current.delete(sessionId);
    lastSeqRef.current.delete(sessionId);
    streamMessageIdsRef.current.delete(sessionId);
  }, []);

  return {
    tabs,
    activeTab,
    activeId,
    createTab,
    createTabFromSession,
    closeTab,
    closeSession,
    selectTab,
    renameTab,
    updateTabSummary,
    updateTab,
    updateSessionSummary,
    // Single-source-of-truth chunk apply + stream lifecycle.
    applyStreamChunk,
    getStreamingMessageId,
    setStreamingMetrics,
    // Stream lifecycle:
    //   - clearTurnStreamState: per-TURN reset (accumulator + shell
    //     id), KEEPS the `lastSeq` watermark. Called on the
    //     stream-done `message_appended(assistant)`.
    //   - clearSessionStreamState: full reset incl. `lastSeq`. Called
    //     when the SESSION ends.
    // Both exported so callers (and tests) can drive the lifecycle.
    clearTurnStreamState,
    clearSessionStreamState,
  };
}

/** Build a ws:// or wss:// URL with the JWT passed as `?token=`. */
function buildWsUrl(path: string, token: string): string {
  const base = (import.meta.env?.VITE_API_BASE as string | undefined) ?? "";
  const isAbsolute = /^https?:\/\//i.test(base);
  const scheme = isAbsolute
    ? base.replace(/^http/i, "ws")
    : `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}`;
  return `${scheme}${path}?token=${encodeURIComponent(token)}`;
}
