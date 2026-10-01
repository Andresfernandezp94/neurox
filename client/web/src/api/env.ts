// API client for /v1/env. EP-0020-02.
//
// El daemon devuelve snake_case (serde) y los tipos del front son
// camelCase. El mapeo vive acá, no en los componentes: si viviera en el
// componente, cada uno que lea un campo distinto se olvida de uno y el
// bug aparece como `undefined` en la UI en vez de como error de tipos.
//
// OJO: los valores de las env vars nunca se devuelven. Solo `set`.

import { apiGet, apiPut, apiDelete } from "./client";
import type {
  EnvResponse,
  EnvVar,
  EnvCategory,
} from "../types";

export type { EnvVar, EnvResponse, EnvCategory };

/** Convierte una variable del daemon al tipo del front. */
function toEnvVar(raw: Record<string, unknown>): EnvVar {
  const key = String(raw.key ?? "");
  return {
    key,
    set: Boolean(raw.set),
    readOnly: Boolean(raw.read_only),
    sensitive: Boolean(raw.sensitive),
    category: String(raw.category ?? "custom"),
    categoryLabel: String(raw.category_label ?? ""),
    description: String(raw.description ?? ""),
    defaultValue:
      raw.default_value === null || raw.default_value === undefined
        ? null
        : String(raw.default_value),
    inFile: Boolean(raw.in_file),
    inEnv: Boolean(raw.in_env),
  };
}

export async function getEnv(): Promise<EnvResponse> {
  const raw = await apiGet<{
    vars?: Record<string, unknown>[];
    unknown_vars?: Record<string, unknown>[];
    categories?: Record<string, unknown>[];
    path?: string;
  }>("/v1/env");
  return {
    path: raw.path ?? "",
    vars: (raw.vars ?? []).map(toEnvVar),
    unknownVars: (raw.unknown_vars ?? []).map(toEnvVar),
    categories: (raw.categories ?? []).map((c) => ({
      id: String(c.id ?? ""),
      label: String(c.label ?? ""),
      description: String(c.description ?? ""),
    })) as EnvCategory[],
  };
}

export async function putEnvVar(key: string, value: string): Promise<void> {
  await apiPut(`/v1/env/${encodeURIComponent(key)}`, { value });
}

export async function deleteEnvVar(key: string): Promise<void> {
  await apiDelete(`/v1/env/${encodeURIComponent(key)}`);
}