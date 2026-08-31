// MCPs API client — dynamic MCP (formerly "plugin") registration.

import { apiGet, apiPost } from "./client";

export interface PluginInfo {
  name: string;
  base_url: string;
  status: "connected" | "disconnected" | "error";
  tools: string[];
  skills: string[];
  health_path: string;
  registered_at: string | null;
  last_health_check: string | null;
}

export interface CatalogEntry {
  id: string;
  repo: string;
  description: string;
  latest_version: string;
  installed: boolean;
  installed_in_daemon: boolean;
  artifact: string | null;
  sha256: string | null;
}

export interface PluginsCatalogResponse {
  installed: string[];
  registry_url: string | null;
  plugins: Record<string, CatalogEntry>;
  error: string | null;
}

export interface ReconnectResult {
  reconnected: string;
  error?: string;
}

/** GET /v1/mcps */
export async function getPlugins(): Promise<PluginInfo[]> {
  const data = await apiGet<{ plugins: PluginInfo[] }>("/v1/mcps");
  return data.plugins;
}

/** GET /v1/mcps/catalog — list every MCP in the configured registry
 *  plus a flag indicating whether it is currently installed in this
 *  daemon. Used by the PluginsPanel to render an installable catalog
 *  alongside the installed list. */
export async function getPluginsCatalog(): Promise<PluginsCatalogResponse> {
  return apiGet<PluginsCatalogResponse>("/v1/mcps/catalog");
}

/** POST /v1/mcps/:name/reconnect */
export async function reconnectPlugin(name: string): Promise<ReconnectResult> {
  return apiPost<ReconnectResult>(`/v1/mcps/${name}/reconnect`, {});
}
