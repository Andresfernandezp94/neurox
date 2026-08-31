// API client for /v1/env. EP-0020-02.

import { apiGet, apiPut, apiDelete } from "./client";
import type { EnvResponse, EnvVar } from "../types";

export type { EnvVar, EnvResponse };

export async function getEnv(): Promise<EnvResponse> {
  return apiGet<EnvResponse>("/v1/env");
}

export async function putEnvVar(key: string, value: string): Promise<void> {
  await apiPut(`/v1/env/${encodeURIComponent(key)}`, { value });
}

export async function deleteEnvVar(key: string): Promise<void> {
  await apiDelete(`/v1/env/${encodeURIComponent(key)}`);
}
