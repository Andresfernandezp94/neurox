// LLM provider API client — EP-0009-06 + EP-0010 + EP-0018-04.

import { apiGet, apiPost, apiPut, apiDelete } from "./client";

/** Runtime state of a local service (EP-0018-03). Mirrors the daemon's
 *  `ServiceState` slug. `null` for remote providers (no local service). */
export type ServiceState = "stopped" | "starting" | "running" | "ready" | "failed";

export interface LlmProviderStatus {
  id: string;
  kind: string;
  model: string;
  base_url: string;
  api_key_env?: string | null;
  configured: boolean;
  active: boolean;
  /** EP-0018-04: only present for local providers. Remote providers omit it
   *  or send `null`. The admin uses it to render badges + Start/Stop buttons. */
  service_state?: ServiceState | null;
  /** EP-0018-01: local service orchestration fields. Present only when the
   *  provider has `local_command` configured. */
  local_command?: string | null;
  local_args?: string[];
  local_model_path?: string | null;
  local_port?: number | null;
}

export interface TestResult {
  ok: boolean;
  provider_id?: string;
  latency_ms?: number;
  response_preview?: string;
  error?: string;
}

export interface DiscoveredModel {
  id: string;
  owned_by?: string | null;
  /** Heuristic capability bucket from the daemon (kebab-case). */
  capability?:
    | "text-generation"
    | "chat"
    | "embeddings"
    | "image"
    | "vision"
    | "audio"
    | "tts"
    | "transcription"
    | "code"
    | "rerank"
    | "unknown";
}

export interface CreateProviderPayload {
  id: string;
  kind: string;
  base_url?: string;
  model?: string;
  api_key_env?: string;
  extra?: Record<string, string>;
  /** EP-0018-01: local service orchestration fields. */
  local_command?: string;
  local_args?: string[];
  local_model_path?: string;
  local_port?: number;
}

export interface UpdateProviderPayload {
  base_url?: string;
  model?: string;
  api_key_env?: string;
  extra?: Record<string, string>;
  /** EP-0018-01: local service orchestration fields. */
  local_command?: string;
  local_args?: string[];
  local_model_path?: string;
  local_port?: number;
}

/** Provider list response (EP-0017-02 R2): `default_provider`/`default_model`
 *  are the daemon fallback used by sessions without an explicit model. */
export interface ProvidersResponse {
  providers: LlmProviderStatus[];
  default_provider: string;
  default_model: string;
}

/** GET /v1/llm/providers — list providers + daemon default model. */
export async function getProviders(): Promise<ProvidersResponse> {
  return apiGet<ProvidersResponse>("/v1/llm/providers");
}

/** POST /v1/llm/providers — create a new provider */
export async function addProvider(payload: CreateProviderPayload): Promise<LlmProviderStatus> {
  return apiPost<LlmProviderStatus>("/v1/llm/providers", payload);
}

/** PUT /v1/llm/providers/:id — update an existing provider */
export async function updateProvider(
  id: string,
  payload: UpdateProviderPayload,
): Promise<LlmProviderStatus> {
  return apiPut<LlmProviderStatus>(`/v1/llm/providers/${id}`, payload);
}

/** DELETE /v1/llm/providers/:id — remove a provider */
export async function deleteProvider(id: string): Promise<void> {
  await apiDelete(`/v1/llm/providers/${id}`);
}

/** PUT /v1/llm/providers/active — set the active provider */
export async function setActiveProvider(providerId: string): Promise<void> {
  await apiPut("/v1/llm/providers/active", { provider_id: providerId });
}

/** POST /v1/llm/providers/:id/test — test provider connectivity */
export async function testProvider(providerId: string): Promise<TestResult> {
  return apiPost<TestResult>(`/v1/llm/providers/${providerId}/test`, {});
}

/** POST /v1/llm/providers/:id/ping — ping provider (store-backed, with optional model) */
export async function pingProvider(
  providerId: string,
  model?: string,
): Promise<TestResult> {
  const body = model ? { model } : {};
  return apiPost<TestResult>(`/v1/llm/providers/${providerId}/ping`, body);
}

// ─── EP-0018-04 — local service start/stop (orchestrator) ────────────────────

/** Response shape for start/stop of a local provider (EP-0018-03). */
export interface LocalServiceToggleResponse {
  id: string;
  service_state: ServiceState;
}

/** POST /v1/llm/providers/:id/start — start the local service. */
export async function startProvider(
  providerId: string,
): Promise<LocalServiceToggleResponse> {
  return apiPost<LocalServiceToggleResponse>(
    `/v1/llm/providers/${encodeURIComponent(providerId)}/start`,
    {},
  );
}

/** POST /v1/llm/providers/:id/stop — stop the local service. */
export async function stopProvider(
  providerId: string,
): Promise<LocalServiceToggleResponse> {
  return apiPost<LocalServiceToggleResponse>(
    `/v1/llm/providers/${encodeURIComponent(providerId)}/stop`,
    {},
  );
}

/** GET /v1/llm/providers/:id/models — discover available models.
 *
 * The daemon returns `models` as a plain array of model-id STRINGS
 * (e.g. `["MiniMax-M3", ...]`), not objects. Older/local providers may
 * still return `{id, capability, ...}` objects. We normalize both shapes
 * into `DiscoveredModel` so callers always get a well-formed `.id`.
 * Entries with no usable id are dropped (prevents `undefined.localeCompare`
 * crashes downstream in ModelSelector's sort). */
export async function listModels(providerId: string): Promise<DiscoveredModel[]> {
  const data = await apiGet<{ provider_id: string; models: unknown[] }>(
    `/v1/llm/providers/${providerId}/models`,
  );
  const raw = Array.isArray(data.models) ? data.models : [];
  const normalized: DiscoveredModel[] = [];
  for (const m of raw) {
    if (typeof m === "string") {
      if (m.length > 0) normalized.push({ id: m });
    } else if (m && typeof m === "object") {
      const obj = m as { id?: unknown; model_id?: unknown };
      const id =
        typeof obj.id === "string"
          ? obj.id
          : typeof obj.model_id === "string"
            ? obj.model_id
            : undefined;
      if (id) normalized.push({ ...(m as object), id } as DiscoveredModel);
    }
  }
  return normalized;
}

// ─── EP-0017-03 — accumulated model catalog + per-session selection ────────

/** One entry in the accumulated catalog returned by `GET /v1/llm/models`. */
export interface CatalogModel {
  provider_id: string;
  model_id: string;
  kind: string;
  base_url: string;
  /** Heuristic capability bucket from the daemon (kebab-case). */
  capability?: string;
  /** Heuristic: model id matches a known tool-supporting family. */
  supports_tools?: boolean;
  /** Heuristic: model id contains "free". */
  is_free?: boolean;
}

/** Response of `GET /v1/llm/models`. */
export interface ModelCatalogResponse {
  models: CatalogModel[];
  cached: boolean;
  fetched_at: string;
}

/** GET /v1/llm/models — accumulated catalog across configured providers. */
export async function getModelCatalog(): Promise<ModelCatalogResponse> {
  return apiGet<ModelCatalogResponse>("/v1/llm/models");
}

// ─── EP-0018-05 — Local GGUF model discovery (read-only) ────────────────

/** One GGUF file in the daemon's models directory (`NEUROX_MODELS_DIR`). */
export interface LocalModel {
  filename: string;
  path: string;
  size_bytes: number;
}

/** Response of `GET /v1/llm/models/local`. */
export interface LocalModelsResponse {
  models: LocalModel[];
  /** Resolved absolute path of the models directory, or null if missing. */
  dir: string | null;
  /** Name of the env var that controls the directory. */
  env_var: string;
}

/** GET /v1/llm/models/local — list GGUF files in the daemon's models dir. */
export async function getLocalModels(): Promise<LocalModelsResponse> {
  return apiGet<LocalModelsResponse>("/v1/llm/models/local");
}

/** PUT /v1/sessions/:id/model — set per-session provider+model (EP-0017-02 R3). */
export async function setSessionModel(
  sessionId: string,
  providerId: string,
  model: string,
): Promise<{ session_id: string; provider_id: string; model: string }> {
  return apiPut(`/v1/sessions/${encodeURIComponent(sessionId)}/model`, {
    provider_id: providerId,
    model,
  });
}

/** PUT /v1/env/:key — set an env var persistently. */
export async function putEnvVar(
  key: string,
  value: string,
): Promise<{ ok: boolean; key: string; persisted: boolean }> {
  return apiPut(`/v1/env/${encodeURIComponent(key)}`, { value });
}
