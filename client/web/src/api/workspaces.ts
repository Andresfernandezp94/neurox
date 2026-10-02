// API de workspaces — entornos aislados (EP-0026-UX).
//
// Reemplaza el mock que estaba hardcodeado en `WorkspaceViewer` (el objeto
// "Sixbell"). Cada workspace tiene su directorio raiz y su propio sandbox;
// `readable_paths` / `writable_paths` se guardan SIN resolver, asi que
// `${workspace}` lo expande el daemon contra la raiz de ESE workspace.
//
// El mapeo snake_case -> camelCase vive aca y no en los componentes, por
// el mismo motivo que en `env.ts`: si vive en el componente, cada uno que
// consume el endpoint tiene que acordarse.

import { apiDelete, apiGet, apiPatch, apiPost, apiPut } from "./client";

export interface WorkspaceSandbox {
  enabled: boolean;
  writable_paths: string[];
  readable_paths: string[];
  max_recursion_depth: number;
}

export interface Workspace {
  id: string;
  name: string;
  description: string | null;
  root: string;
  status: "active" | "paused" | "draft";
  icon: string | null;
  is_default: boolean;
  sandbox: WorkspaceSandbox;
  created_at: string;
  updated_at: string;
}

/** Lo que se puede mandar al crear o editar. Todo opcional salvo al crear. */
export interface WorkspaceInput {
  name?: string;
  root?: string;
  description?: string | null;
  status?: Workspace["status"];
  icon?: string | null;
  is_default?: boolean;
  sandbox_enabled?: boolean;
  sandbox_writable_paths?: string[];
  sandbox_readable_paths?: string[];
  sandbox_max_recursion_depth?: number;
}

export interface SandboxDefaults {
  defaults: {
    enabled: boolean;
    writable_paths: string[];
    readable_paths: string[];
    max_recursion_depth: number;
  };
  statuses: Workspace["status"][];
}

/** El daemon ya responde en snake_case y en camelCase; se normaliza igual. */
function toWorkspace(raw: Record<string, unknown>): Workspace {
  const s = (raw.sandbox ?? {}) as Record<string, unknown>;
  return {
    id: String(raw.id ?? ""),
    name: String(raw.name ?? ""),
    description: (raw.description as string | null) ?? null,
    root: String(raw.root ?? ""),
    status: (raw.status as Workspace["status"]) ?? "active",
    icon: (raw.icon as string | null) ?? null,
    is_default: Boolean(raw.is_default ?? false),
    sandbox: {
      enabled: Boolean(s.enabled ?? true),
      writable_paths: (s.writable_paths as string[]) ?? [],
      readable_paths: (s.readable_paths as string[]) ?? [],
      max_recursion_depth: Number(s.max_recursion_depth ?? 10),
    },
    created_at: String(raw.created_at ?? ""),
    updated_at: String(raw.updated_at ?? ""),
  };
}

export async function listWorkspaces(): Promise<Workspace[]> {
  const res = await apiGet<{ workspaces: Record<string, unknown>[] }>("/v1/workspaces");
  return res.workspaces.map(toWorkspace);
}

export async function getWorkspace(id: string): Promise<Workspace> {
  const res = await apiGet<{ workspace: Record<string, unknown> }>(
    `/v1/workspaces/${encodeURIComponent(id)}`,
  );
  return toWorkspace(res.workspace);
}

export async function createWorkspace(input: WorkspaceInput): Promise<Workspace> {
  const res = await apiPost<{ workspace: Record<string, unknown> }>("/v1/workspaces", input);
  return toWorkspace(res.workspace);
}

export async function updateWorkspace(
  id: string,
  patch: WorkspaceInput,
): Promise<Workspace> {
  const res = await apiPatch<{ workspace: Record<string, unknown> }>(
    `/v1/workspaces/${encodeURIComponent(id)}`,
    patch,
  );
  return toWorkspace(res.workspace);
}

export async function deleteWorkspace(id: string): Promise<void> {
  await apiDelete<void>(`/v1/workspaces/${encodeURIComponent(id)}`);
}

/**
 * Parchea el sandbox de UN workspace. Distinto de `putSandbox`, que mueve el
 * global: el global sigue siendo el default de las sesiones sin workspace.
 */
export async function putWorkspaceSandbox(
  id: string,
  patch: WorkspaceInput,
): Promise<Workspace> {
  const res = await apiPut<{ workspace: Record<string, unknown> }>(
    `/v1/workspaces/${encodeURIComponent(id)}/sandbox`,
    patch,
  );
  return toWorkspace(res.workspace);
}

/**
 * Defaults con los que se crea un workspace.
 *
 * Se piden al daemon en vez de hardcodear `${workspace}` en el form: si el
 * default del backend cambia, el form lo sigue sin tener que tocarlo.
 */
export async function getSandboxDefaults(): Promise<SandboxDefaults> {
  return apiGet<SandboxDefaults>("/v1/workspaces/sandbox-defaults");
}