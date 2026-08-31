// API client for /v1/default/status. EP-0020-02.

import { apiGet } from "./client";
import type { DefaultAgentResponse } from "../types";

export type { DefaultAgentResponse };

export async function getDefaultAgentStatus(): Promise<DefaultAgentResponse> {
  return apiGet<DefaultAgentResponse>("/v1/default/status");
}
