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
import type { Message } from "../types";
import type { ModelSelection } from "../components/ModelSelector";
import { apiGet, apiPost, getToken } from "../api/client";
import { getWebClientId } from "../shared/clientId";

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
  const wsRef = useRef<WebSocket | null>(null);

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
        setTabs((prev) => {
          const next = prev.filter((t) => t.sessionId !== sid);
          if (activeId && !next.some((t) => t.id === activeId) && next.length > 0) {
            setActiveId(next[0]!.id);
          } else if (next.length === 0) {
            setActiveId(null);
          }
          return next;
        });
      } else if (type === "message_appended" && sid) {
        // Other device (or this one on reload) added a message to a
        // session. Mirror it into the local tab's transcript with
        // dedup by `message_id` (which the daemon mints on insert).
        const message_id = evt.message_id as number | undefined;
        const role = (evt.role as string) ?? "user";
        const content = (evt.content as string) ?? "";
        const ts = (evt.ts as string) ?? new Date().toISOString();
        const thinking = evt.thinking as string | undefined;
        if (message_id == null) return;
        setTabs((prev) => {
          const tab = prev.find((t) => t.sessionId === sid);
          if (!tab) return prev;
          if (tab.messages.some((m) => (m as { id: number }).id === message_id)) {
            return prev; // already have this message — dedupe
          }
          return prev.map((t) =>
            t.id === tab.id
              ? {
                  ...t,
                  messages: [
                    ...t.messages,
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
        (type === "content" || type === "thinking") &&
        sid &&
        typeof evt.text === "string"
      ) {
        // Remote stream chunk (another device is talking). Append to
        // the tab's last assistant message (creating one if absent).
        const text = evt.text;
        const chunkKind = type === "content" ? "content" : "thinking";
        setTabs((prev) => {
          const tab = prev.find((t) => t.sessionId === sid);
          if (!tab) return prev;
          return prev.map((t) => {
            if (t.id !== tab.id) return t;
            const msgs = t.messages;
            // Find the last assistant message; create one if none.
            const lastAssistant = [...msgs].reverse().find(
              (m) => m.role === "assistant",
            );
            const newMessage: Message = lastAssistant ?? {
              id: Date.now(),
              session_id: sid,
              role: "assistant",
              content: "",
              timeline: [],
              ts: new Date().toISOString(),
            };
            const timelineEntry =
              chunkKind === "content"
                ? { type: "content" as const, text }
                : { type: "thinking" as const, text };
            const merged: Message = lastAssistant
              ? {
                  ...lastAssistant,
                  content:
                    chunkKind === "content"
                      ? (lastAssistant.content ?? "") + text
                      : lastAssistant.content ?? "",
                  thinking:
                    chunkKind === "thinking"
                      ? (lastAssistant.thinking ?? "") + text
                      : lastAssistant.thinking,
                  timeline: [
                    ...(lastAssistant.timeline ?? []),
                    timelineEntry,
                  ],
                }
              : newMessage;
            const replaced =
              lastAssistant !== undefined
                ? msgs.map((m) => (m === lastAssistant ? merged : m))
                : [...msgs, merged];
            return { ...t, messages: replaced };
          });
        });
      }
    };
    wsRef.current = ws;
    return () => {
      ws.close();
      wsRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId]);

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
