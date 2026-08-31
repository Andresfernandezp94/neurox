// Workspace data model.
//
// A workspace is a named config that groups:
//   - sandbox(es) — filesystem permissions
//   - network policy — egress rules
//   - env vars — scoped environment
//   - resources — memory / cpu / disk / time limits
//   - tools — allow/deny lists
//   - MCPs — active plugins + per-MCP config
//   - skills — custom skill instructions
//   - agents — list of agent IDs that use this workspace
//
// Note: the LLM model (`provider` + `model`) is a per-agent setting,
// not a workspace setting — it lives on the agent config so different
// agents in the same workspace can use different models.

import type { IconName } from "../shared/components/atoms/Icon";

export type Permission = "read" | "write" | "execute" | "delete";

export interface SandboxRule {
  /** Path or `${workspace}/relative` placeholder. */
  path: string;
  /** Permissions granted on this path. */
  permissions: Permission[];
  /** If true, the rule is recursively applied to sub-paths. */
  recursive?: boolean;
}

export interface NetworkPolicy {
  /** When true, the workspace is fully offline. */
  noNetwork?: boolean;
  /** Allow-list (regex or exact host). Empty = no constraint when
   * `noNetwork` is false. */
  allow?: string[];
  /** Deny-list. */
  deny?: string[];
}

export interface ResourceLimits {
  /** Memory in MB. 0 = unlimited. */
  memoryMb?: number;
  /** CPU cores. 0 = unlimited. */
  cpuCores?: number;
  /** Disk in MB. 0 = unlimited. */
  diskMb?: number;
  /** Per-tool-call timeout in seconds. */
  timeoutSecs?: number;
}

export type EnvValue = string | { secret: true; value: string };

export interface McpConfig {
  enabled: boolean;
  /** Per-MCP overrides (e.g. timeout, base_url). */
  config: Record<string, string>;
}

export type ToolPermission = "allow" | "deny";

export interface ToolPolicy {
  /** Whitelist of tool names (case-insensitive glob like "fs_*"). */
  allow?: string[];
  /** Blacklist. Wins over `allow` if both match. */
  deny?: string[];
}

export interface Workspace {
  id: string;
  name: string;
  description?: string;
  status?: "active" | "paused" | "draft";
  /** Optional icon name from the IconName enum (e.g. "IconIntegrations"). */
  icon?: IconName;

  /** Optional parent workspace id — this workspace inherits the
   * parent's settings and overrides only the fields it specifies. */
  parent?: string;

  sandboxes: SandboxRule[];
  network: NetworkPolicy;
  env: Record<string, EnvValue>;
  resources: ResourceLimits;
  tools: ToolPolicy;
  mcps: Record<string, McpConfig>;
  skills: string[];
  agents: string[];
}

/** Step-1 default shape — used by the editor form when starting
 *  a new workspace. The user fills in fields incrementally. */
export function emptyWorkspace(name: string): Workspace {
  return {
    id: name.toLowerCase().replace(/[^a-z0-9-]+/g, "-"),
    name,
    description: "",
    status: "draft",
    icon: "IconIntegrations",
    sandboxes: [],
    network: { noNetwork: false, allow: [], deny: [] },
    env: {},
    resources: {},
    tools: { allow: [], deny: [] },
    mcps: {},
    skills: [],
    agents: [],
  };
}
