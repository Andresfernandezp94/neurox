// API client for /v1/llm/models/* endpoints. EP-0020-02.

import { apiGet, apiPost, apiPut } from "./client";
import type {
  LocalModel,
  LocalModelsResponse,
  HfModel,
  HfSearchResponse,
  ModelConfig,
} from "../types";

export type { LocalModel, HfModel, ModelConfig, LocalModelsResponse, HfSearchResponse };

export async function getLocalModels(): Promise<LocalModelsResponse> {
  return apiGet<LocalModelsResponse>("/v1/llm/models/local");
}

export async function searchHfModels(query: string): Promise<HfSearchResponse> {
  return apiGet<HfSearchResponse>(
    `/v1/llm/models/hf?search=${encodeURIComponent(query)}`,
  );
}

export async function downloadModel(id: string): Promise<{ ok: boolean }> {
  return apiPost<{ ok: boolean }>("/v1/llm/models/download", { id });
}

/**
 * `GET /v1/llm/models/local/:filename/config`
 *
 * El daemon devuelve la FILA (`{ config, filename, updated_at }`), no el
 * `ModelConfig` pelado, así que hay que desenvolverla. Sin esto los campos
 * salían todos `undefined` y el panel se veía vacío incluso para un modelo
 * que ya tenía config guardada.
 *
 * Sin fila guardada devuelve `{}`: todos los campos en `None` significa "todo
 * por defecto", que es el estado inicial de cualquiera.
 */
export async function getModelConfig(filename: string): Promise<ModelConfig> {
  const res = await apiGet<{ config?: ModelConfig } | ModelConfig>(
    `/v1/llm/models/local/${encodeURIComponent(filename)}/config`,
  );
  // El daemon puede devolver la fila envuelta o el config pelado (el caso
  // "sin fila" devuelve `{}`). Se distinguen por la presencia de `config`.
  if (res && typeof res === "object" && "config" in res) {
    return (res as { config?: ModelConfig }).config ?? {};
  }
  return (res as ModelConfig) ?? {};
}

/**
 * `PUT /v1/llm/models/local/:filename/config`
 *
 * El body va ENVOLTO en `{ config }`: el daemon deserializa
 * `PutModelConfigReq { config: ModelConfig }`. Mandarlo plano devolvía 422
 * ("missing field `config`") y la configuración de sampling no se guardaba
 * nunca — el panel se llenaba, el Save respondía error, y nada persistía.
 *
 * La respuesta trae la fila guardada (`{config, filename, updated_at}`), no
 * el `ModelConfig` pelado, así que se desenvuelve acá.
 */
export async function putModelConfig(
  filename: string,
  config: ModelConfig,
): Promise<ModelConfig> {
  const res = await apiPut<{ config?: ModelConfig }>(
    `/v1/llm/models/local/${encodeURIComponent(filename)}/config`,
    { config },
  );
  return res.config ?? {};
}
