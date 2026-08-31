// API client for /v1/tools. EP-0004 + EP-0020-02.

import { apiGet } from "./client";
import type { ToolsResponse } from "../types";

export function listTools(): Promise<ToolsResponse> {
  return apiGet<ToolsResponse>("/v1/tools");
}

/** EP-0020-02: alias for new code that prefers the longer name. */
export const getTools = listTools;
