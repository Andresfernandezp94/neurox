// Cliente WebSocket persistente para /v1/commands. EP-0003-05.
// Implementa el protocolo bidireccional del daemon:
// - Cliente envía `WsCommand` con un `request_id` generado.
// - Servidor responde con `WsResponse` que contiene el mismo `request_id`.
// - request_id se mantiene en un Map<id, {resolve, reject, timeout}>.
// - Auto-reconnect con backoff exponencial.
// - HTTP fallback se hace en el caller (no aquí).
//
// Uso:
//   const client = getCommandsClient();
//   client.init();   // idempotente
//   if (client.isOpen()) {
//     const res = await client.send({ type: 'start_agent', id: 'default' });
//   } else {
//     // HTTP fallback
//   }

import type { ApprovalDecision } from './approvals';
import { getApiBase } from './client';

export type WsCommand =
  | { type: 'list_agents' }
  | { type: 'start_agent'; id: string }
  | { type: 'stop_agent'; id: string }
  | { type: 'cancel_session'; session_id: string }
  | { type: 'approval_response'; id: string; decision: ApprovalDecision };

export type WsResponse =
  | { type: 'agent_list'; persistent: unknown[]; ephemeral_templates: unknown[]; running: unknown[] }
  | { type: 'agent_started'; id: string; ok: boolean }
  | { type: 'agent_stopped'; id: string; ok: boolean }
  | { type: 'session_cancelled'; session_id: string; was_active: boolean }
  | { type: 'approval_resolved'; id: string; decision: string }
  | { type: 'pong' }
  | { type: 'error'; message: string };

interface PendingRequest {
  resolve: (value: WsResponse) => void;
  reject: (reason: Error) => void;
  timer: ReturnType<typeof setTimeout>;
  sentAt: number;
}

const DEFAULT_COMMAND_TIMEOUT_MS = 10_000;
const DEFAULT_PATH = '/v1/commands';
const DEFAULT_INITIAL_BACKOFF_MS = 1_000;
const DEFAULT_MAX_BACKOFF_MS = 30_000;

export class CommandsClient {
  private socket: WebSocket | null = null;
  private path: string;
  private initialBackoffMs: number;
  private maxBackoffMs: number;
  private commandTimeoutMs: number;
  private backoff: number;
  private manuallyClosed = false;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private pending = new Map<string, PendingRequest>();
  private requestCounter = 0;
  private latencySamples: number[] = [];

  constructor(opts: {
    path?: string;
    initialBackoffMs?: number;
    maxBackoffMs?: number;
    commandTimeoutMs?: number;
  } = {}) {
    this.path = opts.path ?? DEFAULT_PATH;
    this.initialBackoffMs = opts.initialBackoffMs ?? DEFAULT_INITIAL_BACKOFF_MS;
    this.maxBackoffMs = opts.maxBackoffMs ?? DEFAULT_MAX_BACKOFF_MS;
    this.commandTimeoutMs = opts.commandTimeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS;
    this.backoff = this.initialBackoffMs;
  }

  /** Inicializa la conexión. Idempotente. Llamar después del primer render. */
  init(): void {
    if (this.socket || typeof WebSocket === 'undefined') return;
    this.connect();
  }

  isOpen(): boolean {
    return this.socket?.readyState === WebSocket.OPEN;
  }

  /** Latencia promedio móvil de los últimos 10 comandos. */
  getLatencyMs(): number | null {
    if (this.latencySamples.length === 0) return null;
    const sum = this.latencySamples.reduce((a, b) => a + b, 0);
    return Math.round(sum / this.latencySamples.length);
  }

  send<T extends WsResponse = WsResponse>(cmd: WsCommand): Promise<T> {
    if (!this.isOpen()) {
      return Promise.reject(new Error('commands WS not open'));
    }
    const id = this.nextRequestId();
    const sentAt = Date.now();
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`command timeout after ${this.commandTimeoutMs}ms`));
      }, this.commandTimeoutMs);
      this.pending.set(id, {
        resolve: (v) => {
          this.recordLatency(Date.now() - sentAt);
          resolve(v as T);
        },
        reject,
        timer,
        sentAt,
      });
      try {
        this.socket!.send(JSON.stringify({ ...cmd, request_id: id }));
      } catch (e) {
        this.pending.delete(id);
        clearTimeout(timer);
        reject(e as Error);
      }
    });
  }

  close(): void {
    this.manuallyClosed = true;
    if (this.retryTimer !== null) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    this.socket?.close();
    this.socket = null;
    for (const [id, p] of this.pending) {
      clearTimeout(p.timer);
      p.reject(new Error('commands client closed'));
      this.pending.delete(id);
    }
  }

  private nextRequestId(): string {
    this.requestCounter += 1;
    return `cmd-${Date.now()}-${this.requestCounter}`;
  }

  private recordLatency(ms: number): void {
    this.latencySamples.push(ms);
    if (this.latencySamples.length > 10) this.latencySamples.shift();
  }

  private connect(): void {
    if (this.manuallyClosed) return;
    if (typeof WebSocket === 'undefined') return;
    let apiBase = '';
    try {
      // getApiBase() throws in production builds without VITE_API_BASE.
      // The dev fallback to window.location.host is preserved.
      apiBase = getApiBase();
    } catch {
      apiBase = '';
    }
    const wsUrl = this.path.startsWith('ws')
      ? this.path
      : apiBase
        ? `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${apiBase.replace(/^https?:\/\//, '')}${this.path}`
        : `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}${this.path}`;

    const socket = new WebSocket(wsUrl);
    this.socket = socket;

    socket.addEventListener('open', () => {
      this.backoff = this.initialBackoffMs;
    });

    socket.addEventListener('message', (ev: MessageEvent<string>) => {
      let parsed: WsResponse & { request_id?: string };
      try {
        parsed = JSON.parse(ev.data);
      } catch {
        return;
      }
      const reqId = parsed.request_id;
      if (!reqId) return; // No es una respuesta a un comando nuestro.
      const pending = this.pending.get(reqId);
      if (!pending) return;
      this.pending.delete(reqId);
      clearTimeout(pending.timer);
      if (parsed.type === 'error') {
        pending.reject(new Error((parsed as { message?: string }).message ?? 'unknown error'));
      } else {
        pending.resolve(parsed);
      }
    });

    socket.addEventListener('close', () => {
      this.socket = null;
      // Rechazar pending
      for (const [id, p] of this.pending) {
        clearTimeout(p.timer);
        p.reject(new Error('commands WS closed'));
        this.pending.delete(id);
      }
      if (!this.manuallyClosed) {
        const wait = this.backoff;
        this.backoff = Math.min(this.backoff * 2, this.maxBackoffMs);
        this.retryTimer = setTimeout(() => this.connect(), wait);
      }
    });

    socket.addEventListener('error', () => {
      socket.close();
    });
  }
}

// ─── Singleton ──────────────────────────────────────────────────────────

let singleton: CommandsClient | null = null;

export function getCommandsClient(): CommandsClient {
  if (!singleton) {
    singleton = new CommandsClient();
  }
  return singleton;
}
