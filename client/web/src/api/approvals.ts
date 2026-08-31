import { apiGet, apiPost } from './client';
import { getCommandsClient } from './commands';
import type { ApprovalsResponse } from '../types';

export type ApprovalDecision = 'approve' | 'deny' | 'approve_always';

export function listApprovals(): Promise<ApprovalsResponse> {
  return apiGet<ApprovalsResponse>('/v1/approvals');
}

/** EP-0003-05: prefiere /v1/commands WS si está abierto, fallback a HTTP. */
export async function respondApproval(
  id: string,
  decision: ApprovalDecision,
): Promise<unknown> {
  const c = getCommandsClient();
  if (c.isOpen()) {
    try {
      const res = await c.send({ type: 'approval_response', id, decision });
      return res;
    } catch (e) {
      // Fallback a HTTP.
    }
  }
  return apiPost(`/v1/approvals/${encodeURIComponent(id)}/respond`, { decision });
}
