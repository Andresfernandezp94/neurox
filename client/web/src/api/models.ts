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

export async function getModelConfig(filename: string): Promise<ModelConfig> {
  return apiGet<ModelConfig>(
    `/v1/llm/models/local/${encodeURIComponent(filename)}/config`,
  );
}

export async function putModelConfig(
  filename: string,
  config: ModelConfig,
): Promise<ModelConfig> {
  return apiPut<ModelConfig>(
    `/v1/llm/models/local/${encodeURIComponent(filename)}/config`,
    config,
  );
}
