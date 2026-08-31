// ApprovalNode — pending/resolved approval row in the chat timeline.

import { useCallback, useState } from "react";
import { respondApproval } from "../../api/approvals";
import type { ApprovalActivity } from "../../types";

export function ApprovalNode({
  approval,
}: {
  approval: ApprovalActivity;
  /** @deprecated no longer needed — respondApproval() reads the
   *  current session from the Bearer token on its own. Kept for
   *  backwards-compatible callers (TimelineRenderer). */
  sessionId?: string | null;
}) {
  const [responding, setResponding] = useState(false);
  const [respondError, setRespondError] = useState<string | null>(null);

  const respond = useCallback(
    async (decision: "approve" | "deny") => {
      if (!approval.id) return;
      setResponding(true);
      setRespondError(null);
      try {
        // respondApproval() prefers the /v1/commands WS (lower latency)
        // and falls back to HTTP via apiPost() which adds the Bearer
        // token. Using bare fetch() here previously caused 401s under
        // auth_required daemons — the request would silently fail and
        // the agent would hang waiting for the approval that never
        // landed.
        await respondApproval(approval.id, decision);
      } catch (e) {
        setRespondError((e as Error).message);
      } finally {
        setResponding(false);
      }
    },
    [approval.id],
  );

  const resolved = !!approval.decision;
  const decision = approval.decision;

  return (
    <div className={`chat__timeline-node chat__timeline-node--approval${resolved ? " chat__timeline-node--resolved" : ""}`}>
      <span
        className={`chat__timeline-icon chat__timeline-icon--approval${resolved ? ` chat__timeline-icon--${decision}` : ""}`}
        aria-label={resolved ? `Approval ${decision}` : "Awaiting approval"}
      >
        {resolved ? (decision === "approve" ? "✓" : "✗") : "⚠"}
      </span>
      <div className="chat__timeline-body">
        <div className="chat__timeline-header">
          <span className="chat__timeline-label chat__timeline-label--tool">{approval.tool}</span>
          <span
            className={`chat__timeline-detail chat__timeline-detail--${resolved ? (decision === "approve" ? "ok" : "bad") : "warn"}`}
          >
            {resolved
              ? decision === "approve"
                ? "✓ approved"
                : "✗ denied"
              : approval.reason ?? "tool requires approval"}
          </span>
        </div>
        {!resolved && (
          <div className="chat__timeline-result-preview chat__timeline-approval-actions">
            <button
              type="button"
              className="chat__timeline-approval-btn chat__timeline-approval-btn--approve"
              onClick={() => void respond("approve")}
              disabled={responding}
              data-testid="approval-approve"
            >
              ✓ Approve
            </button>
            <button
              type="button"
              className="chat__timeline-approval-btn chat__timeline-approval-btn--deny"
              onClick={() => void respond("deny")}
              disabled={responding}
              data-testid="approval-deny"
            >
              ✗ Deny
            </button>
          </div>
        )}
        {respondError && (
          <div
            className="chat__timeline-approval-error"
            role="alert"
            data-testid="approval-respond-error"
          >
            {respondError}
          </div>
        )}
      </div>
    </div>
  );
}
