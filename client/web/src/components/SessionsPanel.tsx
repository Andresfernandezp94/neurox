// SessionsPanel — panel dedicado "Sessions" en el sidebar.
// Lista full-width con SessionList variant="expanded".
// Al seleccionar una sesión, se abre SessionDetailModal encima.
//
// EP-0028-b: live via the store (WS session_started / session_ended).
// `onStop` (toggle) and `onDelete` (real DELETE) are wired through.

import { useCallback, useEffect, useState } from "react";
import {
  listSessions,
  getSessionMessages,
  cancelSession,
  deleteSession,
  renameSession,
} from "../api/sessions";
import { useStore } from "../store/StoreContext";
import { SessionList } from "../shared/components/SessionList";
import { SessionDetailModal } from "../shared/components/SessionDetailModal";
import { Panel } from "../shared/components/molecules/Panel";
import type { Message, MessagesResponse, SessionSummary } from "../types";

export function SessionsPanel() {
  // EP-0028-b: derive sessions from the global store for live updates.
  const { state } = useStore();
  const sessions: SessionSummary[] = Array.from(state.sessions.values()).sort(
    (a, b) => (b.started_at ?? "").localeCompare(a.started_at ?? ""),
  );
  const loading = !state.loaded.sessions && sessions.length === 0;

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      await listSessions();
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const openSession = useCallback(async (id: string) => {
    setSelectedId(id);
    setLoadingMessages(true);
    setError(null);
    try {
      const res: MessagesResponse = await getSessionMessages(id);
      setMessages(res.messages ?? []);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoadingMessages(false);
    }
  }, []);

  const closeModal = useCallback(() => {
    setSelectedId(null);
    setMessages([]);
  }, []);

  const handleRename = useCallback(
    async (id: string, name: string) => {
      await renameSession(id, name);
      // WS will broadcast the updated session.
    },
    [],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      // EP-0028-b: real DELETE — kills the agent and removes the
      // session record from the daemon. Previously this called
      // cancelSession (POST /cancel), which only stops the agent
      // but leaves the session in the list.
      try {
        await deleteSession(id);
        if (selectedId === id) closeModal();
      } catch (e) {
        setError((e as Error).message);
      }
    },
    [selectedId, closeModal],
  );

  const handleStop = useCallback(
    async (id: string) => {
      // Stop the agent but keep the session record (message log
      // stays). The row's status dot will go from green to grey.
      try {
        await cancelSession(id);
      } catch (e) {
        setError((e as Error).message);
      }
    },
    [],
  );

  const selectedSession = selectedId
    ? sessions.find((s) => s.session_id === selectedId) ?? null
    : null;

  return (
    <Panel className="sessions-panel">
      {error && <div className="error-banner">Error: {error}</div>}

      <SessionList
        sessions={sessions}
        currentSessionId={selectedId}
        variant="expanded"
        loading={loading}
        onLoad={openSession}
        onRename={handleRename}
        onDelete={handleDelete}
        onStop={handleStop}
        headerLabel="Sessions"
      />

      <SessionDetailModal
        sessionId={selectedId}
        session={selectedSession}
        messages={messages}
        loading={loadingMessages}
        onClose={closeModal}
      />
    </Panel>
  );
}
