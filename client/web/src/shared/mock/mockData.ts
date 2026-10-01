// mockData.ts — datos de mentira para developing la UI sin daemon.
//
// Objetivo: poder diseñar la página inicial (HomePage) y las vistas de
// admin sin levantar el backend. Cuando el daemon esté conectado, esto
// NO se usa: los datos reales siempre ganan.
//
// Fuente única de mock data. Si activás `VITE_NEUROX_MOCK`, el store se
// hidrata desde acá en lugar de pegarle al daemon (ver
// `isMockMode()` y `mockSnapshots()`).

import type { Agent, Health, SessionSummary } from "../../types";

/** Flag de modo mock. Se lee en build-time (Vite la inyecta). */
export function isMockMode(): boolean {
  return import.meta.env.VITE_NEUROX_MOCK === "true";
}

/** Health simulado. Mismo shape que el `GET /health` real. */
export const MOCK_HEALTH: Health = {
  service: "neurox",
  status: "ok",
  version: "0.4.0-mock",
  started_at: new Date(Date.now() - 3_600_000).toISOString(),
  uptime_seconds: 3_600,
  auth_required: false,
};

/** Latencia simulada en ms. */
export const MOCK_LATENCY_MS = 18;

/** Agentes simulados — mix de persistentes, efímeros e in-process. */
export const MOCK_AGENTS: Agent[] = [
  {
    id: "default",
    kind: "ephemeral",
    status: "idle",
    protocol: "json-rpc",
    transport: "stdio",
  },
  {
    id: "researcher",
    kind: "ephemeral",
    status: "idle",
    protocol: "json-rpc",
    transport: "stdio",
  },
  {
    id: "doc-agent",
    kind: "ephemeral",
    status: "idle",
    protocol: "json-rpc",
    transport: "stdio",
  },
  {
    id: "orchestrator",
    kind: "in_process",
    status: "running",
  },
];

/** Sesiones simuladas. `session_id` es la key del Map en el store. */
export const MOCK_SESSIONS: SessionSummary[] = [
  {
    session_id: "sess-mock-01",
    agent_id: "default",
    provider_id: "minimax",
    model: "minimax-m2",
    started_at: new Date(Date.now() - 120_000).toISOString(),
    ended_at: null,
    summary: "Refactor del tools engine",
    client_id: "web",
  },
  {
    session_id: "sess-mock-02",
    agent_id: "researcher",
    provider_id: "anthropic",
    model: "claude-sonnet-5",
    started_at: new Date(Date.now() - 900_000).toISOString(),
    ended_at: new Date(Date.now() - 600_000).toISOString(),
    summary: "Auditoría de seguridad",
    client_id: "web",
  },
  {
    session_id: "sess-mock-03",
    agent_id: "doc-agent",
    provider_id: "openrouter",
    model: "auto",
    started_at: new Date(Date.now() - 5_400_000).toISOString(),
    ended_at: null,
    summary: "Migración de serde_yaml",
    client_id: "sidebar-ii",
  },
  {
    session_id: "sess-mock-04",
    agent_id: "default",
    provider_id: "mistral",
    model: "mistral-large",
    started_at: new Date(Date.now() - 86_400_000).toISOString(),
    ended_at: new Date(Date.now() - 82_800_000).toISOString(),
    summary: null,
    client_id: "web",
  },
];

/** Aprobaciones pendientes simuladas. */
export const MOCK_APPROVALS = [
  {
    id: "apr-mock-01",
    tool: "shell",
    args: { command: "cargo build --release" },
    requested_at: new Date().toISOString(),
  },
] as const;

/**
 * Aplica los snapshots mock al store. Mapea 1:1 con las acciones que
 * dispara la respuesta real del daemon, para que los reducers y los
 * tests ejerciten exactamente el mismo camino de código.
 */
export function mockSnapshots() {
  return {
    health: MOCK_HEALTH,
    agents: MOCK_AGENTS,
    sessions: MOCK_SESSIONS,
    approvals: [...MOCK_APPROVALS],
    latencyMs: MOCK_LATENCY_MS,
  };
}