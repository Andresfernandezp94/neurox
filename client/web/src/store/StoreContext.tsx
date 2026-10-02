// Store global del admin. EP-0003-03.
// Estado compartido por todos los paneles: agents, sessions, approvals,
// connection. Una sola suscripción a /v1/events. Reducer puro testeable.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type Dispatch,
  type ReactNode,
} from 'react';
import type {
  Agent,
  Approval,
  DaemonEvent,
  Health,
  SessionSummary,
} from '../types';
import { apiGet, getApiBase, getToken } from '../api/client';
import { getCommandsClient } from '../api/commands';

// ─── Tipos ──────────────────────────────────────────────────────────────

export type WsStatus = 'connecting' | 'open' | 'closed';

export interface ConnectionState {
  ws: WsStatus;
  health: Health | null;
  latencyMs: number | null;
  lastPongAt: number | null;
  isZombie: boolean;
  retriesTotal: number;
  lastRetryAt: number | null;
}

export interface StoreState {
  agents: Map<string, Agent>;
  sessions: Map<string, SessionSummary>;
  approvals: Map<string, Approval>;
  connection: ConnectionState;
  loaded: {
    agents: boolean;
    sessions: boolean;
    approvals: boolean;
    health: boolean;
  };
}

export type StoreAction =
  | { type: 'EVENT_RECEIVED'; event: DaemonEvent }
  | { type: 'SNAPSHOT_AGENTS'; agents: Agent[] }
  | { type: 'SNAPSHOT_SESSIONS'; sessions: SessionSummary[] }
  | { type: 'SNAPSHOT_APPROVALS'; approvals: Approval[] }
  | { type: 'SNAPSHOT_HEALTH'; health: Health }
  | { type: 'WS_STATUS_CHANGED'; status: WsStatus }
  | { type: 'HEARTBEAT_TICK'; latencyMs: number | null; lastPongAt: number | null; isZombie: boolean }
  | { type: 'HTTP_RETRY_RECORDED'; retriesTotal: number; lastRetryAt: number | null }
  | { type: 'AGENT_LOCAL_UPDATE'; id: string; patch: Partial<Agent> }
  | { type: 'APPROVAL_LOCAL_UPDATE'; id: string; patch: Partial<Approval> }
  | { type: 'AGENT_LOCAL_REMOVE'; id: string }
  | { type: 'APPROVAL_LOCAL_REMOVE'; id: string };

// ─── Estado inicial ─────────────────────────────────────────────────────

export const initialState: StoreState = {
  agents: new Map(),
  sessions: new Map(),
  approvals: new Map(),
  connection: {
    ws: 'connecting',
    health: null,
    latencyMs: null,
    lastPongAt: null,
    isZombie: false,
    retriesTotal: 0,
    lastRetryAt: null,
  },
  loaded: {
    agents: false,
    sessions: false,
    approvals: false,
    health: false,
  },
};

// ─── Reducer ────────────────────────────────────────────────────────────

export function reducer(state: StoreState, action: StoreAction): StoreState {
  switch (action.type) {
    case 'SNAPSHOT_AGENTS': {
      const next = new Map<string, Agent>();
      for (const a of action.agents) next.set(a.id, a);
      return { ...state, agents: next, loaded: { ...state.loaded, agents: true } };
    }
    case 'SNAPSHOT_SESSIONS': {
      // EP-2026-08-15: el daemon hace soft-delete (preserva historial
      // con summary="deleted" y ended_at seteado). Filtramos esas
      // sesiones acá para que la UI las trate como inexistentes; si
      // el user abre /v1/sessions a mano, siguen apareciendo ahí.
      const next = new Map<string, SessionSummary>();
      for (const s of action.sessions) {
        if (s.summary === 'deleted') continue;
        next.set(s.session_id, s);
      }
      return { ...state, sessions: next, loaded: { ...state.loaded, sessions: true } };
    }
    case 'SNAPSHOT_APPROVALS': {
      const next = new Map<string, Approval>();
      for (const a of action.approvals) next.set(a.id, a);
      return { ...state, approvals: next, loaded: { ...state.loaded, approvals: true } };
    }
    case 'SNAPSHOT_HEALTH': {
      return {
        ...state,
        connection: { ...state.connection, health: action.health },
        loaded: { ...state.loaded, health: true },
      };
    }
    case 'WS_STATUS_CHANGED': {
      return {
        ...state,
        connection: { ...state.connection, ws: action.status },
      };
    }
    case 'HEARTBEAT_TICK': {
      return {
        ...state,
        connection: {
          ...state.connection,
          latencyMs: action.latencyMs,
          lastPongAt: action.lastPongAt,
          isZombie: action.isZombie,
        },
      };
    }
    case 'HTTP_RETRY_RECORDED': {
      return {
        ...state,
        connection: {
          ...state.connection,
          retriesTotal: action.retriesTotal,
          lastRetryAt: action.lastRetryAt,
        },
      };
    }
    case 'AGENT_LOCAL_UPDATE': {
      const prev = state.agents.get(action.id);
      if (!prev) return state;
      const next = new Map(state.agents);
      next.set(action.id, { ...prev, ...action.patch });
      return { ...state, agents: next };
    }
    case 'AGENT_LOCAL_REMOVE': {
      if (!state.agents.has(action.id)) return state;
      const next = new Map(state.agents);
      next.delete(action.id);
      return { ...state, agents: next };
    }
    case 'APPROVAL_LOCAL_UPDATE': {
      const prev = state.approvals.get(action.id);
      if (!prev) return state;
      const next = new Map(state.approvals);
      next.set(action.id, { ...prev, ...action.patch });
      return { ...state, approvals: next };
    }
    case 'APPROVAL_LOCAL_REMOVE': {
      if (!state.approvals.has(action.id)) return state;
      const next = new Map(state.approvals);
      next.delete(action.id);
      return { ...state, approvals: next };
    }
    case 'EVENT_RECEIVED': {
      return reduceEvent(state, action.event);
    }
    default:
      return state;
  }
}

// Mapea eventos del WS a mutaciones del store.
// Solo manejamos los eventos que afectan entidades del store.
// El resto se ignora (LiveEventsPanel los renderiza como log).
function reduceEvent(state: StoreState, event: DaemonEvent): StoreState {
  const t = event.type;
  switch (t) {
    case 'agent_spawned': {
      const id = String(event.ephemeral_id ?? event.agent_id ?? '');
      if (!id) return state;
      const next = new Map(state.agents);
      next.set(id, {
        ...(state.agents.get(id) ?? { id }),
        id,
        kind: 'ephemeral',
        status: 'running',
        type: event.agent_id as string,
      });
      return { ...state, agents: next };
    }
    case 'agent_finished': {
      const id = String(event.ephemeral_id ?? '');
      if (!id || !state.agents.has(id)) return state;
      const prev = state.agents.get(id)!;
      const next = new Map(state.agents);
      next.set(id, { ...prev, status: String(event.status ?? 'finished') });
      return { ...state, agents: next };
    }
    case 'session_started': {
      const id = String(event.session_id ?? '');
      if (!id) return state;
      const next = new Map(state.sessions);
      next.set(id, {
        session_id: id,
        agent_id: String(event.agent_id ?? ''),
        started_at: new Date().toISOString(),
        ended_at: null,
        summary: null,
      });
      return { ...state, sessions: next };
    }
    case 'session_ended': {
      const id = String(event.session_id ?? '');
      if (!id) return state;
      const next = new Map(state.sessions);
      // EP-2026-08-15: si el summary es "deleted" (DELETE explícito del
      // usuario), removemos la sesión del Map. El backend la conserva
      // internamente como soft-delete (historial), pero el cliente no la
      // muestra más en ningún panel.
      if ((event.summary as string | null) === 'deleted') {
        next.delete(id);
      } else {
        const prev = state.sessions.get(id);
        next.set(id, {
          ...(prev ?? { session_id: id, agent_id: '', started_at: new Date().toISOString() }),
          ended_at: new Date().toISOString(),
          summary: (event.summary as string | null) ?? null,
        });
      }
      return { ...state, sessions: next };
    }
    case 'approval_request': {
      const req = event.request as { id?: string; tool?: string; args?: unknown; requested_at?: string } | undefined;
      const id = String(req?.id ?? '');
      if (!id) return state;
      const next = new Map(state.approvals);
      next.set(id, {
        id,
        tool: String(req?.tool ?? ''),
        args: req?.args,
        requested_at: req?.requested_at,
      });
      return { ...state, approvals: next };
    }
    case 'approval_resolved': {
      const id = String(event.approval_id ?? '');
      if (!id || !state.approvals.has(id)) return state;
      const next = new Map(state.approvals);
      next.delete(id);
      return { ...state, approvals: next };
    }
    default:
      return state;
  }
}

// ─── Context ────────────────────────────────────────────────────────────

/**
 * Evento crudo del WS `/v1/events`, ya parseado.
 *
 * Se pasa el objeto entero y no un tipo por variante: los consumidores
 * (hoy `useChatTabs`) filtran por `type` y leen lo que necesitan, y asi
 * agregar un evento al daemon no obliga a tocar el store.
 */
export type DaemonEventPayload = Record<string, unknown>;

export interface StoreContextValue {
  state: StoreState;
  dispatch: React.Dispatch<StoreAction>;
  snapshot: () => Promise<void>;
  /** Selector con shallow-equal para evitar re-renders innecesarios. */
  select: <T>(selector: (s: StoreState) => T) => T;
  /**
   * Registra un listener de eventos del WS. Devuelve la baja.
   *
   * La app tiene UN solo socket a `/v1/events`, el de este provider, con
   * backoff y reconexion. Antes `useChatTabs` abria un socket propio al
   * mismo endpoint y SIN ninguna reconexion: si ese se caia (daemon
   * reiniciado, red dormida) el chat dejaba de recibir eventos en
   * silencio, mientras el indicador de conexion decia "conectado" porque
   * el otro socket si reconectaba.
   *
   * Se entrega el evento directo por callback y no por estado a proposito:
   * pasar por estado perderia eventos entre renders, y los chunks del
   * stream son ordenados y no se pueden volver a pedir.
   */
  subscribeEvents: (fn: (evt: DaemonEventPayload) => void) => () => void;
}

const StoreContext = createContext<StoreContextValue | null>(null);

// ─── Provider ───────────────────────────────────────────────────────────

export interface StoreProviderProps {
  children: ReactNode;
  /** Path del WS de eventos (default '/v1/events'). */
  eventsPath?: string;
}

export function StoreProvider({ children, eventsPath = '/v1/events' }: StoreProviderProps) {
  // Listeners de eventos del WS. Un Set para que la baja sea O(1) y para
  // que el mismo handler no se registre dos veces.
  const eventListenersRef = useRef<Set<(evt: DaemonEventPayload) => void>>(new Set());
  const subscribeEvents = useCallback((fn: (evt: DaemonEventPayload) => void) => {
    eventListenersRef.current.add(fn);
    return () => {
      eventListenersRef.current.delete(fn);
    };
  }, []);
  const [state, dispatch] = useReducer(reducer, initialState);

  // EP-0003-05: inicializa el cliente de /v1/commands (singleton).
  useEffect(() => {
    getCommandsClient().init();
  }, []);

  // Snapshot inicial: 4 requests en paralelo al montar.
  const snapshot = useCallback(async () => {
    const [agentsR, sessionsR, approvalsR, healthR] = await Promise.allSettled([
      apiGet<{ persistent: Agent[]; ephemeral_templates: Agent[]; running: Agent[]; in_process: Agent[] }>(
        '/v1/agents',
      ),
      apiGet<{ sessions: SessionSummary[] }>('/v1/sessions'),
      apiGet<{ pending: Approval[] }>('/v1/approvals'),
      apiGet<Health>('/health'),
    ]);

    if (agentsR.status === 'fulfilled') {
      const all: Agent[] = [
        ...(agentsR.value.in_process ?? []),
        ...(agentsR.value.persistent ?? []),
        ...(agentsR.value.ephemeral_templates ?? []),
      ];
      dispatch({ type: 'SNAPSHOT_AGENTS', agents: all });
    }
    if (sessionsR.status === 'fulfilled') {
      dispatch({ type: 'SNAPSHOT_SESSIONS', sessions: sessionsR.value.sessions });
    }
    if (approvalsR.status === 'fulfilled') {
      dispatch({ type: 'SNAPSHOT_APPROVALS', approvals: approvalsR.value.pending });
    }
    if (healthR.status === 'fulfilled') {
      dispatch({ type: 'SNAPSHOT_HEALTH', health: healthR.value });
    }
  }, []);

  // Suscripción al WS de eventos (single source of truth).
  useEffect(() => {
    if (typeof WebSocket === 'undefined') return;
    let apiBase = '';
    try {
      // getApiBase() throws in production builds without VITE_API_BASE.
      // Dev fallbacks to window.location.host are preserved.
      apiBase = getApiBase();
    } catch {
      apiBase = '';
    }
    const baseWsUrl = eventsPath.startsWith('ws')
      ? eventsPath
      : apiBase
        ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${apiBase.replace(/^https?:\/\//, '')}${eventsPath}`
        : `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}${eventsPath}`;

    // EP-2026-09-02: el daemon exige JWT en `?token=` para el upgrade WS
    // (los browsers no permiten headers custom en WS). Lo leemos fresh en
    // cada connect() para que un login reabra la conexión sin esperar el
    // backoff. VITE_API_BASE ya viene encoded en baseWsUrl, solo el token.
    function buildWsUrl(): string {
      const token = getToken();
      if (!token) return baseWsUrl;
      const sep = baseWsUrl.includes('?') ? '&' : '?';
      return `${baseWsUrl}${sep}token=${encodeURIComponent(token)}`;
    }

    let socket: WebSocket | null = null;
    let backoff = 1000;
    let retryTimer: ReturnType<typeof setTimeout> | null = null;
    let manuallyClosed = false;
    let consecutiveFailures = 0;
    let wantImmediateRetry = false;

    function connect() {
      if (manuallyClosed) return;
      socket = new WebSocket(buildWsUrl());
      dispatch({ type: 'WS_STATUS_CHANGED', status: 'connecting' });

      socket.addEventListener('open', () => {
        backoff = 1000;
        consecutiveFailures = 0;
        dispatch({ type: 'WS_STATUS_CHANGED', status: 'open' });
        // Re-snapshot después de un reconnect (estado pudo cambiar).
        if (state.loaded.agents || state.loaded.sessions) {
          void snapshot();
        }
      });

      socket.addEventListener('message', (ev: MessageEvent<string>) => {
        try {
          const parsed = JSON.parse(ev.data) as DaemonEvent;
          dispatch({ type: 'EVENT_RECEIVED', event: parsed });
          // Se reparte a los listeners registrados (hoy `useChatTabs`).
          // Se itera sobre una copia: un listener puede darse de baja
          // durante la llamada (si desmonta mientras llega el evento) y
          // mutar el Set mientras se recorre.
          for (const fn of Array.from(eventListenersRef.current)) {
            try {
              fn(parsed as unknown as DaemonEventPayload);
            } catch (e) {
              // Un listener que tira no puede cortar el WS ni impedir que
              // los demas reciban el evento.
              console.error('event listener failed', e);
            }
          }
        } catch {
          // ignore
        }
      });

      socket.addEventListener('close', () => {
        dispatch({ type: 'WS_STATUS_CHANGED', status: 'closed' });
        socket = null;
        if (!manuallyClosed) {
          if (wantImmediateRetry) {
            // EP-2026-09-02: disparado por neurox:auth-changed. Saltamos
            // el backoff y reintentamos ya con el token fresh.
            wantImmediateRetry = false;
            backoff = 1000;
            retryTimer = setTimeout(connect, 0);
          } else {
            retryTimer = setTimeout(connect, backoff);
            backoff = Math.min(backoff * 2, 30_000);
          }
          consecutiveFailures += 1;
        }
      });

      socket.addEventListener('error', () => {
        socket?.close();
      });
    }

    // EP-2026-09-02: cuando el user hace login o logout, useAuth dispara
    // `neurox:auth-changed`. Cerramos el socket actual (si lo hay) para
    // forzar el reconnect inmediato con el token fresh; el close handler
    // ve `wantImmediateRetry` y no aplica backoff.
    function onAuthChanged() {
      wantImmediateRetry = true;
      const s = socket;
      if (!s) return;
      if (s.readyState === WebSocket.OPEN) {
        s.close();
      } else if (s.readyState === WebSocket.CONNECTING) {
        setTimeout(() => s.close(), 0);
      }
    }
    window.addEventListener('neurox:auth-changed', onAuthChanged);

    connect();
    void snapshot();

    return () => {
      manuallyClosed = true;
      window.removeEventListener('neurox:auth-changed', onAuthChanged);
      if (retryTimer !== null) clearTimeout(retryTimer);
      // EP-0023-04 (2026-08-12): React 18 StrictMode en dev ejecuta el
      // mount-unmount-mount inmediato. El primer socket no ha abierto
      // todavía cuando el cleanup lo cierra, generando "WebSocket is
      // closed before the connection is established". Peor: si el socket
      // aún no abrió, llamar .close() puede dejar al socket en estado
      // CONNECTING y nunca dispararse el 'close' handler — entonces
      // manuallyClosed=true bloquea la reconexión del segundo mount.
      // Solución: diferir el close a un microtask. Si el socket ya abrió,
      // se cierra normal; si está CONNECTING, se cierra antes del handshake
      // y dispara el close handler que detecta manuallyClosed y no reintenta
      // (correcto: el segundo mount del StrictMode creará su propio socket).
      const s = socket;
      if (s && s.readyState === WebSocket.CONNECTING) {
        // El socket todavía no abrió. Cerrar manualmente disparará el
        // close event. Pero esperamos al siguiente tick para no interferir
        // con el handshake que está en vuelo.
        setTimeout(() => s.close(), 0);
      } else {
        s?.close();
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [eventsPath]);

  // Selector con cache por símbolo: misma entrada → mismo valor, sin re-render
  // si el valor no cambió por referencia.
  const select = useCallback(<T,>(selector: (s: StoreState) => T) => {
    // Para v1: retornamos el valor directo. El cache shallow-equal es
    // una optimización que se puede agregar después sin breaking change.
    return selector(state);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state]);

  const value = useMemo<StoreContextValue>(
    () => ({ state, dispatch, snapshot, select, subscribeEvents }),
    [state, snapshot, select, subscribeEvents],
  );

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

// ─── Hook ───────────────────────────────────────────────────────────────

export function useStore(): StoreContextValue {
  const ctx = useContext(StoreContext);
  if (!ctx) throw new Error('useStore must be used within a StoreProvider');
  return ctx;
}

/** Hook de conveniencia para despachar acciones al store.
 *  Lo usan las fuentes externas al provider (ej. el modo mock de App.tsx). */
export function useStoreDispatch(): Dispatch<StoreAction> {
  const ctx = useContext(StoreContext);
  if (!ctx) throw new Error('useStoreDispatch must be used within a StoreProvider');
  return ctx.dispatch;
}

/** Hook de conveniencia para el indicador de conexión. */
export function useConnectionState(): ConnectionState {
  return useStore().state.connection;
}
