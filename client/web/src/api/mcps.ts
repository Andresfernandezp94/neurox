// MCPs API client — dynamic MCP (formerly "plugin") registration.

import { apiGet, apiPost } from "./client";

/**
 * Un tool publicado por el motor de tools.
 *
 * OJO: `GET /v1/mcps` NO devuelve servidores plugin registrados sino el
 * catálogo de tools (`{"plugins": [...]}` en el daemon, ver
 * `http::list_plugins` → `state.engine.tools.list_specs()`). El shape
 * anterior (`base_url`, `status`, `health_path`) no existe en la
 * respuesta real: el daemon computa `mcp_plugins` pero nunca lo incluye
 * en el JSON, y no hay endpoint `/v1/plugins`. Este tipo refleja lo que
 * la API devuelve hoy, no lo que se asumía.
 */
export interface McpToolSpec {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  requires_approval: boolean;
  categories: string[];
  mode_compatible: string[];
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

/** GET /v1/mcps — catálogo de tools expuestas por el motor. */
export async function getPlugins(): Promise<McpToolSpec[]> {
  const data = await apiGet<{ plugins: McpToolSpec[] }>("/v1/mcps");
  return data.plugins ?? [];
}

/** Alias con nombre explícito, para no volver a confundirlo con
 *  servidores plugin. */
export const getMcpTools = getPlugins;

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
