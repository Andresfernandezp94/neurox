// ChatPanel — agent-studio inspired: timeline layout, typewriter streaming,
// thinking as collapsible timeline node (not a bubble).
// EP-0024: tabs arriba (vía useChatTabs) — múltiples sesiones en paralelo.
// Refactor: dividido en ChatHeader + ChatMain + ChatFooter.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useChatTabs } from "../hooks/useChatTabs";
import { useDefaultAgentId } from "../hooks/useDefaultAgentId";
import { useNotifications } from "../store/NotificationsContext";
import {
  cancelSession,
  getSessionMessages,
  listSessions,
} from "../api/sessions";
import { getDefaultAgentStatus, type DefaultAgentResponse } from "../api/default";
import { countTranscriptMatches } from "./chat/searchTranscript";
import type { Message, MessageMetrics } from "../types";
import { useAppFullscreen } from "../shared/hooks/useAppFullscreen";
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
    ensureSessionForTab,
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
  /**
   * Los errores van al stack de notificaciones, no a estado local del
   * panel. Antes eran un unico string: el segundo error pisaba al primero
   * y no habia forma de apilarlos, asi que un turno que fallaba dos veces
   * (una tool, despues la llamada siguiente al LLM) solo mostraba la
   * ultima, y sin forma de cerrarla.
   */
  const notify = useNotifications();
  const [showHistory, setShowHistory] = useState(false);
  // EP-0026-UX: "Fullscreen real con sidebar" — el botón de maximizar
  // usa la Fullscreen API sobre el shell `.app` (sidebar + chat). Así el
  // chat ocupa todo el viewport como antes, PERO la sidebar queda visible
  // (vive dentro del elemento fullscreen). En browsers sin la API (p.ej.
  // iOS Safari) cae a la clase CSS `chat-layout--focus` (mismo resultado
  // visual, sin Fullscreen API).
  //
  // El estado NO es local: vive en `useAppFullscreen`, un store a nivel de
  // modulo, porque hay un segundo control del mismo estado en la barra de
  // mobile (que vive en el Sidebar, fuera de este arbol). Con estado local
  // los dos iconos podrian contradecirse.
  const chatLayoutRef = useRef<HTMLDivElement | null>(null);
  const { isExpanded, isFocusMode, toggle: toggleExpanded } = useAppFullscreen();

  // Sync fallback class on chat-layout root
  useEffect(() => {
    const el = chatLayoutRef.current;
    if (el) {
      el.classList.toggle("chat-layout--focus", isFocusMode);
    }
  }, [isFocusMode]);
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
    // Contenido: no-op. El WS es la unica fuente que aplica chunks (ver
    // el comentario de arriba); aplicar aca duplicaria cada fragmento.
    //
    // Error: si se atiende. El daemon emite `Event::Error` cuando un tool
    // falla o el agente se cae a mitad de turno, y con un no-op total ese
    // evento no llegaba a ninguna parte: el usuario veia el stream
    // cortarse sin ninguna explicación. Solo se levanta el error, no se
    // aplica ningun contenido, asi que no se reintroduce la duplicacion.
    onChunk: useCallback((raw: unknown) => {
      if (
        raw !== null &&
        typeof raw === "object" &&
        (raw as { type?: unknown }).type === "error"
      ) {
        const message = (raw as { message?: unknown }).message;
        notify.error(
          typeof message === "string" && message ? message : "The agent reported an error.",
        );
      }
    }, [notify]),
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

  // La sesion NO se crea al activar la pestana: se crea en `handleSend`,
  // cuando el usuario manda el primer mensaje.
  //
  // Antes este efecto creaba la sesion en cuanto la pestana se activaba, y
  // `createTab` tambien la creaba al abrirla: dos caminos, los dos antes de
  // que hubiera un solo mensaje. Como una sesion sin mensajes queda
  // `ended_at = NULL` para siempre, cada pestana abierta sin escribir
  // dejaba una sesion activa permanente, y la hidratacion las convertia en
  // ventanas de chat al recargar (9 sesiones, 8 de ellas vacias).
  //
  // El comentario anterior decia que el textarea quedaba deshabilitado con
  // placeholder "Connecting…" hasta que `sessionId` estuviera listo. Era
  // cierto, pero el `disabled` vivia en `ChatFooter`, no acá — por eso el
  // gateo se zafaba de una vista. Con la sesion perezosa ese gateo era un
  // deadlock: sin sesion no se puede escribir, y sin escribir no hay sesion.
  // Ahora el footer solo se gatea por `isStreaming`.

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
  //
  // Solo cuando se cambia de una pestana a OTRA. En la carga inicial
  // `activeId` pasa de `null` al id de la primera pestana (la que crea el
  // efecto de draft al terminar la hidratacion), y eso no es un cambio de
  // pestana: es la inicializacion. Si se limpiara ahi, se borraria lo que
  // el usuario hubiera escrito mientras se hidrataba.
  //
  // Antes esto no se notaba porque el textarea estaba deshabilitado hasta
  // que existia la sesion, con lo cual era imposible escribir durante esa
  // ventana. Con la sesion perezosa el textarea es usable desde el
  // principio, asi que el borrado se vuelve visible: escribir y perder el
  // texto.
  const prevActiveIdRef = useRef<string | null>(null);
  useEffect(() => {
    const prev = prevActiveIdRef.current;
    prevActiveIdRef.current = activeId;
    // `prev === null`: primera asignacion (oMontaje, o la pestana activa
    // se cerro). `prev === activeId`: React StrictMode remunta el efecto.
    if (prev === null || prev === activeId) return;
    setInput("");
    // El stack de avisos NO se limpia al cambiar de pestana. Un aviso
    // sigue siendo cierto en la otra pestana, y borrarlo hacia que al
    // volver no estuviera nada que mostrar de lo que paso.
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
      // Only APPLY a model when the backend reports one. Do NOT reset
      // to null otherwise: the tab may already carry the correct model
      // (restored by `createTabFromSession` from the session's
      // persisted config, or set by the user). A later lookup that
      // misses the row — stale list, different client_id filter, a
      // just-reactivated session not yet listed — must not wipe it.
      // That "else → null" was what made the model flash in and then
      // reset to the "select model" placeholder.
      if (summary?.provider_id && summary?.model) {
        updateTab(tabId, { sessionModel: { provider_id: summary.provider_id, model: summary.model } });
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
      notify.error(`Failed to load session: ${(e as Error).message}`);
    }
  }, [createTabFromSession, refreshSessionModel, updateTab, notify]);

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
        notify.error(`Failed to load session: ${(e as Error).message}`);
        // Allow retry on next tab switch.
        loadedSessions.current.delete(sid);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab?.id, activeTab?.sessionId, notify]);

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
    if (!text || isStreaming || !activeTab) return;
    // El stack de avisos NO se limpia al mandar un mensaje. Este `clear`
    // venia del estado local de un solo error, donde limpiar al empezar
    // un turno tenia sentido. Con un stack que tiene que permanecer hasta
    // que se lo cierre, hacia que cada turno nuevo borrara los avisos del
    // turno anterior: se veia como que los errores se pisaban entre si en
    // vez de apilarse.

    const tabId = activeTab.id;

    // La sesion se crea recien aca: recien cuando hay un mensaje que
    // justificar su existencia.
    //
    // Antes `handleSend` abortaba si la pestana no tenia `sessionId`, y la
    // sesion la creaba un efecto al activar la pestana. Eso dejaba una
    // sesion activa permanente por cada ventana abierta sin escribir.
    // Ahora se crea bajo demanda y es idempotente.
    let sessionId: string | null = activeTab.sessionId;
    if (!sessionId) {
      try {
        sessionId = await ensureSessionForTab(activeTab);
      } catch (e) {
        notify.error(
          `Could not start the session: ${(e as Error).message}. Your message was not sent.`,
        );
        return;
      }
      if (!sessionId) {
        notify.error("Could not start the session. Your message was not sent.");
        return;
      }
    }

    setInput("");

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
      await sendStream(text, sessionId);
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
      notify.error((e as Error).message);
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
  }, [input, isStreaming, activeTab, sendStream, updateTab, refreshTabSummary, setStreamingMetrics, notify]);

  const handleCancel = useCallback(async () => {
    if (!activeTab?.sessionId) return;
    // Dos cortes en paralelo: cliente (AbortController vía hook) +
    // servidor (cancelSession endpoint). Mismas semantics que antes.
    cancelStream();
    try {
      await cancelSession(activeTab.sessionId);
    } catch (e) {
      notify.error((e as Error).message);
    }
  }, [activeTab?.sessionId, cancelStream, notify]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter envia, Shift+Enter inserta un salto de linea (el
    // comportamiento por defecto del textarea, que no interceptamos).
    //
    // Esto invierte la convencion anterior, que era la de Slack/Discord
    // (Enter = salto, Shift+Enter = enviar) y estaba fijada con tests de
    // regresion. El cambio es pedido explicito del usuario.
    if (e.key !== "Enter") return;

    // Con un IME activo (pinyin, kana, cualquier compositor del sistema),
    // Enter confirma el candidato: es el caracter que el usuario esta
    // escribiendo, no el envio del mensaje. Sin esta excepcion, confirmar
    // la composicion mandaba el texto a medio componer.
    if (e.nativeEvent.isComposing) return;

    // Ctrl/Cmd+Enter tambien envia: no estorba y es lo que se espera
    // de un atajo alternativo.
    if (e.shiftKey && !e.ctrlKey && !e.metaKey) return;

    e.preventDefault();
    void handleSend();
  }, [handleSend]);

  // EP-0028 HMR-fix: `messages`, `sessionId`, and `sessionModel` were
  // moved to the top of the component (right after the refs) so they
  // are in scope when the `searchTotal` useMemo runs. Keeping a
  return (
    <div className="chat-layout" ref={chatLayoutRef}>
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
            notify.error(`Rename failed: ${(e as Error).message}`),
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
        isExpanded={isExpanded}
        onToggleExpanded={toggleExpanded}
        defaultAgent={defaultAgent}
        input={input}
        onInputChange={setInput}
        onKeyDown={handleKeyDown}
        isStreaming={isStreaming}
        onSend={() => void handleSend()}
        onCancel={() => void handleCancel()}
        inputRef={inputRef}
      />
    </div>
  );
}
