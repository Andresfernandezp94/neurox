import { apiGet, apiPost } from "./client";

export interface SkillSummary {
  name: string;
  trigger_keywords: string[];
  instructions_preview: string;
  preferred_tools: string[];
  enabled?: boolean;
}

export type SkillsResponse = SkillSummary[];

export function listSkills(): Promise<SkillsResponse> {
  return apiGet<SkillsResponse>("/v1/skills");
}

export function enableSkill(name: string): Promise<void> {
  return apiPost<void>(`/v1/skills/${encodeURIComponent(name)}/enable`);
}

export function disableSkill(name: string): Promise<void> {
  return apiPost<void>(`/v1/skills/${encodeURIComponent(name)}/disable`);
}
