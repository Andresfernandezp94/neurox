// API client for /v1/services. EP-0004 + EP-0020-02.
//
// El backend retorna {services: ServiceInfo[], clients: ClientInfo[], server_time}.
// StatusPanel.tsx (legacy) usa ambos. ServicesPanel.tsx (EP-0020-02) usa solo
// services — se conserva la misma API para no romper consumidores.

import { apiGet } from "./client";

export interface ServiceInfo {
  id: string;
  name: string;
  description: string;
  kind: string;
  status: 'ok' | 'degraded' | 'error' | 'unreachable' | string;
  version: string;
  endpoint: string;
  latency_ms: number;
  uptime_seconds?: number;
  started_at?: string | null;
  last_checked_at: string;
  error?: string;
  metadata?: Record<string, string>;
}

/** EP-0020-02 subset of ServiceInfo used by ServicesPanel. */
export type ServiceStatus = ServiceInfo;

export interface ClientInfo {
  id: string;
  kind: 'http' | 'ws_events' | 'ws_commands' | string;
  detected_as: 'web' | 'cli' | 'tui' | 'gtk' | 'unknown' | string;
  path: string;
  method: string;
  source_addr: string;
  user_agent: string | null;
  connected_at: string;
  duration_seconds: number;
  requests_count: number;
}

export interface ServicesResponse {
  services: ServiceInfo[];
  clients: ClientInfo[];
  server_time: string;
}

export function listServices(
  opts: { signal?: AbortSignal } = {},
): Promise<ServicesResponse> {
  return apiGet<ServicesResponse>("/v1/services", opts);
}

/** EP-0020-02: alias for new code that prefers the longer name. */
export const getServices = listServices;
