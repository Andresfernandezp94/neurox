// useChatTabs — manages a list of chat tabs (parallel sessions) with
// localStorage persistence. Each tab is a separate session with the
// agent; switching tabs preserves input/messages/session state.
//
// EP-0028: the tab title is kept in sync with the backend session
// `summary` (the same field shown in the History sidebar). This makes
// the visible title a single source of truth — whatever the user sees
// in the History also shows up in the tab header above the chat.
//
// Note on agent resolution: the tab's `sessionAgent` is the
// user-explicit choice (from AgentSelector). On first mount, if the
// daemon's `/v1/agents` snapshot is already available (`defaultAgentId`
// passed in), tabs created from that point use it directly. Tabs that
// were persisted with the empty sentinel (legacy migration, or created
// before the snapshot arrived) are back-filled ONCE when the agent
// id first arrives — after that, the user's explicit choice is
// preserved (we never overwrite a non-empty `sessionAgent`).

import { useCallback, useEffect, useState } from "react";
import type { Message } from "../types";
import type { ModelSelection } from "../components/ModelSelector";
import { renameSession } from "../api/sessions";

export interface ChatTab {
  /** Stable id for the tab (uuid). */
  id: string;
  /** User-visible title. Editable (double-click rename). */
  title: string;
  /** Backend session id (`null` until the backend creates one). */
  sessionId: string | null;
  /** Model override for this session. */
  sessionModel: ModelSelection | null;
  /**
   * Agent for this session. Empty string means "no explicit choice" —
   * the consumer (ChatPanel) resolves the daemon's in-process default
   * at message-send time via `useDefaultAgentId()`. Legacy tabs
   * persisted with `default` or `agent` are migrated to this
   * empty sentinel so the runtime resolver fills them in.
   */
  sessionAgent: string;
  /** Messages exchanged in this tab. */
  messages: Message[];
  /** When the tab was created (for sorting / fallback title). */
  createdAt: number;
  /**
   * `true` once the title was set explicitly by the user (double-click
   * rename). When set, the auto-summary sync must not overwrite it.
   * `false` (default) means the title is either the fallback "Chat N"
   * or a backend summary that may be refreshed as the session evolves.
   */
  manuallyRenamed: boolean;
  /** Search-in-chat: debounced query. Empty string = no active search. */
  searchQuery?: string;
  /** Search-in-chat: index of the currently focused match (0-based). */
  searchActive?: number;
}

const STORAGE_KEY = "chat-tabs-v1";
const ACTIVE_KEY = "chat-active-tab-v1";

// Sentinel for "no explicit agent choice". The consumer resolves the
// daemon's in-process agent at message-send time via `useDefaultAgentId`.
const NO_AGENT_SENTINEL = "";

function isModelSelection(x: unknown): x is ModelSelection | null {
  if (x === null) return true;
  if (typeof x !== "object") return false;
  const o = x as Record<string, unknown>;
  return typeof o.provider_id === "string" && typeof o.model === "string";
}

// Backwards-compat shim: tabs persisted with the legacy `default`
// id get migrated to the empty sentinel so the runtime resolver
// (`useDefaultAgentId`) kicks in on first message. Bump STORAGE_KEY
// once most callers have migrated, then drop this function.
//
// EP-2026-08-19: removed the `"agent"` legacy branch — the agent id
// `agent` no longer exists (per-session subprocesses use `admin`/`user`
// from `session_agents`, no fallback `agent`/`default`). Tabs
// persisted with `agent` will fall through to the runtime resolver
// like any other unknown id.
function normalizeAgentId(x: unknown): string {
  if (typeof x !== "string") return NO_AGENT_SENTINEL;
  if (x === "default") return NO_AGENT_SENTINEL;
  return x;
}

function isValidManuallyRenamed(x: unknown): x is boolean {
  return typeof x === "boolean";
}

function load(): { tabs: ChatTab[]; activeId: string | null } {
  if (typeof window === "undefined") return { tabs: [], activeId: null };
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    const activeId = window.localStorage.getItem(ACTIVE_KEY);
    if (!raw) return { tabs: [], activeId };
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return { tabs: [], activeId };
    const tabs: ChatTab[] = parsed.flatMap((t: unknown) => {
      if (!t || typeof t !== "object") return [];
      const o = t as Record<string, unknown>;
      if (typeof o.id !== "string" || typeof o.title !== "string") return [];
      if (typeof o.createdAt !== "number") return [];
      const sessionId = typeof o.sessionId === "string" ? o.sessionId : null;
      const sessionModel = isModelSelection(o.sessionModel) ? o.sessionModel : null;
      const sessionAgent = normalizeAgentId(o.sessionAgent);
      const messages = Array.isArray(o.messages) ? (o.messages as Message[]) : [];
      // EP-0028: prefer the explicit flag; fall back to a heuristic
      // so tabs persisted before the flag existed still get sane
      // behaviour. Heuristic: if the title doesn't match the "Chat N"
      // pattern AND there's a sessionId, treat it as manually named.
      const manuallyRenamed = isValidManuallyRenamed(o.manuallyRenamed)
        ? o.manuallyRenamed
        : o.sessionId
          ? !/^Chat \d+$/.test(o.title)
          : false;
      const searchQuery = typeof o.searchQuery === "string" ? o.searchQuery : "";
      const searchActive = typeof o.searchActive === "number" ? o.searchActive : 0;
      return [{
        id: o.id,
        title: o.title,
        sessionId,
        sessionModel,
        sessionAgent,
        messages,
        createdAt: o.createdAt,
        manuallyRenamed,
        searchQuery,
        searchActive,
      }];
    });
    return { tabs, activeId: activeId && tabs.some((t) => t.id === activeId) ? activeId : null };
  } catch {
    return { tabs: [], activeId: null };
  }
}

function persist(tabs: ChatTab[], activeId: string | null) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(tabs));
    if (activeId) {
      window.localStorage.setItem(ACTIVE_KEY, activeId);
    } else {
      window.localStorage.removeItem(ACTIVE_KEY);
    }
  } catch {
    // ignore
  }
}

function uuid(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return "tab-" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export function useChatTabs(defaultAgentId: string | null = null) {
  const [tabs, setTabs] = useState<ChatTab[]>(() => load().tabs);
  const [activeId, setActiveId] = useState<string | null>(() => load().activeId);

  // Persist on every change.
  useEffect(() => {
    persist(tabs, activeId);
  }, [tabs, activeId]);

  // EP-0026-01 R1: auto-create a tab on mount if localStorage is
  // empty. The ChatPanel <textarea> reads `sessionId` from
  // activeTab and stays disabled with "Connecting…" placeholder
  // when no tab exists. Without this auto-create, first-time
  // users had to click `+` on the ChatTabs header before they
  // could type.
  useEffect(() => {
    if (tabs.length === 0 && activeId === null) {
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
  }, []);

  // Si por alguna razón no hay tab activa pero hay tabs, activa la primera.
  useEffect(() => {
    if (activeId === null && tabs.length > 0) {
      setActiveId(tabs[0]!.id);
    }
  }, [activeId, tabs]);

  // Back-fill sentinel tabs once the daemon's agent id is available.
  // Only fires when `defaultAgentId` transitions from null → string (or
  // changes between non-null values) AND there are still tabs with
  // `sessionAgent === ""`. Existing explicit choices are never
  // overwritten.
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

  const createTab = useCallback(() => {
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
    return tab;
  }, [tabs.length, defaultAgentId]);

  /**
   * EP-0028: load (or focus) a tab for a given backend session. Used
   * by the History sidebar when the user clicks a past conversation.
   * If a tab already binds this sessionId, just focus it. Otherwise
   * create a new tab pre-populated with the session's summary (so
   * the tab header matches the entry in the History panel).
   */
  const createTabFromSession = useCallback(
    (sessionId: string, summary: string | null) => {
      const existing = tabs.find((t) => t.sessionId === sessionId);
      if (existing) {
        setActiveId(existing.id);
        return existing;
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
        // Loading from History is treated as authoritative: don't let
        // the auto-summary sync overwrite it on the next message.
        manuallyRenamed: true,
        searchQuery: "",
        searchActive: 0,
      };
      setTabs((prev) => [...prev, tab]);
      setActiveId(id);
      return tab;
    },
    [tabs, defaultAgentId],
  );

  const closeTab = useCallback((id: string) => {
    setTabs((prev) => {
      const next = prev.filter((t) => t.id !== id);
      if (next.length === 0) {
        // Always keep at least one tab.
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
  }, [activeId, defaultAgentId]);

  const selectTab = useCallback((id: string) => {
    setActiveId(id);
  }, []);

  /**
   * EP-0028: rename a tab. Persists to the backend (so the History
   * panel shows the same title) and marks the tab as manually
   * renamed (so the auto-summary sync won't overwrite it later).
   * Returns a promise so the caller can await the backend write and
   * surface errors.
   */
  const renameTab = useCallback(
    async (id: string, title: string): Promise<void> => {
      const trimmed = title.trim();
      if (!trimmed) return;

      // Optimistic local update first so the UI responds instantly.
      let sessionId: string | null = null;
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== id) return t;
          sessionId = t.sessionId;
          return { ...t, title: trimmed, manuallyRenamed: true };
        }),
      );

      // Sync to backend only if the tab is bound to a real session.
      // Otherwise (no session yet) the local title is fine and the
      // backend will fill in `summary` once the first message is sent.
      if (sessionId) {
        try {
          await renameSession(sessionId, trimmed);
        } catch (e) {
          // Re-throw so callers can surface the error; the local
          // title is already updated optimistically.
          throw e;
        }
      }
    },
    [],
  );

  /**
   * EP-0028: apply the backend's auto-generated summary to the tab,
   * but ONLY if the user hasn't manually renamed it. Called by the
   * ChatPanel after the first user message of a fresh session.
   */
  const updateTabSummary = useCallback(
    (id: string, summary: string) => {
      const trimmed = summary.trim();
      if (!trimmed) return;
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== id) return t;
          if (t.manuallyRenamed) return t;
          return { ...t, title: trimmed };
        }),
      );
    },
    [],
  );

  /**
   * Apply a partial patch to the tab with the given id.
   *
   * The patch can be either a plain object (simple cases) or a
   * function that receives the current tab and returns the patch
   * to merge. The function form is required when the caller
   * computes a field based on the *latest* tab state — e.g. inside
   * a streaming callback that runs many updates in a row, where a
   * captured `activeTab` would be stale and could cause messages
   * to disappear.
   *
   * Example (streaming chunk handler):
   *   updateTab(activeTab.id, (prev) => ({
   *     messages: prev.messages.map((m) =>
   *       m.id === assistantId ? { ...m, content: streamed } : m
   *     ),
   * }));
   */
  const updateTab = useCallback(
    (
      id: string,
      patch: Partial<ChatTab> | ((prev: ChatTab) => Partial<ChatTab>),
    ) => {
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

  return {
    tabs,
    activeTab,
    activeId,
    createTab,
    createTabFromSession,
    closeTab,
    selectTab,
    renameTab,
    updateTabSummary,
    updateTab,
  };
}