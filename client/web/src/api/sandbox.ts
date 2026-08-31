// API client for /v1/sandbox. EP-0020-02.

import { apiPut } from "./client";
import type { SandboxConfig } from "../types";

export type { SandboxConfig };

/**
 * PUT /v1/sandbox. Empty body returns the current config (so the panel
 * can use the same call to load + save).
 */
export async function putSandbox(
  patch: Partial<SandboxConfig>,
): Promise<SandboxConfig> {
  const res = await apiPut<SandboxConfig>("/v1/sandbox", patch);
  return res;
}
