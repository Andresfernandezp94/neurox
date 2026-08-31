import { apiGet, apiPost } from './client';
import { getCommandsClient } from './commands';
import type { Agent, AgentsResponse } from '../types';

export function listAgents(): Promise<AgentsResponse> {
  return apiGet<AgentsResponse>('/v1/agents');
}

export function getAgent(id: string): Promise<Agent> {
  return apiGet<Agent>(`/v1/agents/${encodeURIComponent(id)}`);
}

/** EP-0003-05: prefiere /v1/commands WS si está abierto, fallback a HTTP. */
export async function startAgent(id: string): Promise<unknown> {
  const c = getCommandsClient();
  if (c.isOpen()) {
    try {
      const res = await c.send({ type: 'start_agent', id });
      if (res.type === 'agent_started' && !res.ok) {
        throw new Error(`start_agent failed for ${id}`);
      }
      return res;
    } catch (e) {
      // Si falla el WS command, fallback a HTTP.
      if (!(e instanceof Error) || !e.message.includes('commands WS not open')) {
        // Continuar con HTTP fallback.
      }
    }
  }
  return apiPost(`/v1/agents/${encodeURIComponent(id)}/start`);
}

export async function stopAgent(id: string): Promise<unknown> {
  const c = getCommandsClient();
  if (c.isOpen()) {
    try {
      const res = await c.send({ type: 'stop_agent', id });
      if (res.type === 'agent_stopped' && !res.ok) {
        throw new Error(`stop_agent failed for ${id}`);
      }
      return res;
    } catch (e) {
      // Fallback a HTTP.
    }
  }
  return apiPost(`/v1/agents/${encodeURIComponent(id)}/stop`);
}
