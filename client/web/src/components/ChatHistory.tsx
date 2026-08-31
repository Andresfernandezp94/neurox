// ChatHistory — right-side aside panel (agent-studio style).
// Wraps the shared SessionList component with sidebar variant.
//
// EP-0028: forwards the backend session summary alongside the id so
// ChatPanel can use it as the new tab's title without an extra roundtrip.
// EP-0028-b: removed the "new chat" header action — new chats start from
// the `+` on the ChatTabs bar instead, so the sidebar stays focused on
// history navigation.

import { useCallback, useEffect } from "react";
import {
  listSessions,
  renameSession,
  cancelSession,
  deleteSession,
} from "../api/sessions";
import { useStore } from "../store/StoreContext";
import { SessionList } from "../shared/components/SessionList";
import type { SessionSummary } from "../types";

interface Props {
  currentSessionId: string | null;
  /**
   * Load a session into the active chat. The optional `summary` is
   * the same field shown in the History list — passing it lets the
   * tab header pick up the title without an extra roundtrip.
   */
  onLoadSession: (sessionId: string, summary: string | null) => void;
  animClass?: string | null;
}

export function ChatHistory({
  currentSessionId,
  onLoadSession,
  animClass,
}: Props) {
  // EP-0028-b: derive sessions directly from the global store. The
  // store is updated live by the daemon's WebSocket
  // (session_started / session_ended) and on each refresh after a
  // rename/delete. No local override — that introduced duplicates
  // when the API and WS disagreed about timing.
  const { state } = useStore();
  const sessions: SessionSummary[] = Array.from(state.sessions.values());

  // First render: nothing in the store yet → brief loading state.
  const loading = !state.loaded.sessions && sessions.length === 0;

  const refresh = useCallback(async () => {
    try {
      await listSessions();
    } catch {
      // silent
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (currentSessionId) void refresh();
  }, [currentSessionId, refresh]);

  const handleRename = useCallback(
    async (sessionId: string, name: string) => {
      await renameSession(sessionId, name);
      // The store will receive a session update via WS / refresh.
    },
    [],
  );

  const handleDelete = useCallback(
    async (sessionId: string) => {
      // EP-0028-b: real DELETE — kills the agent and removes the
      // session record from the daemon. Previously this called
      // cancelSession (POST /cancel), which only stops the agent
      // but leaves the session in the list (so the delete button
      // "didn't work" — it would only cancel if the agent was running).
      await deleteSession(sessionId);
    },
    [],
  );

  const handleStop = useCallback(
    async (sessionId: string) => {
      // Stop the running agent but keep the session record (message
      // log stays). The row's status dot will go from green to grey
      // when the WS broadcasts session_ended.
      await cancelSession(sessionId);
    },
    [],
  );

  // EP-0028: SessionList.onLoad takes only the sessionId, so we
  // resolve the summary here from the cached list and forward to
  // the parent with both pieces.
  const handleLoad = useCallback(
    (sessionId: string) => {
      const match = sessions.find((s) => s.session_id === sessionId);
      onLoadSession(sessionId, match?.summary ?? null);
    },
    [sessions, onLoadSession],
  );

  return (
    <SessionList
      sessions={sessions}
      currentSessionId={currentSessionId}
      variant="sidebar"
      loading={loading}
      onLoad={handleLoad}
      onRename={handleRename}
      onDelete={handleDelete}
      onStop={handleStop}
      className={animClass ?? undefined}
      headerLabel="History"
    />
  );
}
