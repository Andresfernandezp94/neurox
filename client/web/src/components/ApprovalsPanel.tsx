// Panel de Approvals — consume el store global. EP-0003-04.
// Reutiliza ConfirmDialog (refactor). SectionHeader viene de App.tsx.
// Migrated to atomic design in EP-0016 (F4.4).

import { useCallback, useState } from "react";
import { respondApproval } from "../api/approvals";
import { useStore } from "../store/StoreContext";
import { Row } from "../shared/components/molecules/Row";
import { Panel } from "../shared/components/molecules/Panel";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Button } from "../shared/components/atoms/Button";

export function ApprovalsPanel() {
  const { state, dispatch } = useStore();
  const [error, setError] = useState<string | null>(null);

  const pending = Array.from(state.approvals.values());

  const handleRespond = useCallback(
    async (id: string, decision: "approve" | "deny" | "approve_always") => {
      setError(null);
      // Optimistic remove
      dispatch({ type: "APPROVAL_LOCAL_REMOVE", id });
      try {
        await respondApproval(id, decision);
      } catch (e) {
        setError((e as Error).message);
        // No podemos re-insertar sin más info; el próximo snapshot traerá el estado real.
      }
    },
    [dispatch],
  );

  return (
<Panel>
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {pending.length === 0 && (
        <EmptyState>
          <EmptyState.Title>No approvals pending.</EmptyState.Title>
        </EmptyState>
      )}

      <ul className="stack stack--gap-md list-reset">
        {pending.map((a) => (
          <li key={a.id} className="card approval-card">
            <Row justify="between" align="start" gap="md">
              <div className="approval-card__body">
                <div className="code text-md">{a.tool}</div>
                <pre className="code muted text-sm approval-card__args">
                  {JSON.stringify(a.args, null, 2)}
                </pre>
                {a.requested_at && (
                  <div className="muted text-sm approval-card__time">
                    {a.requested_at}
                  </div>
                )}
              </div>
              <Row gap="sm">
                <Button
                  size="sm"
                  variant="ghost"
                  className="approval-card__approve"
                  onClick={() => void handleRespond(a.id, "approve")}
                >
                  Approve
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="approval-card__approve-all"
                  onClick={() => void handleRespond(a.id, "approve_always")}
                >
                  Approve All
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="approval-card__reject"
                  onClick={() => void handleRespond(a.id, "deny")}
                >
                  Deny
                </Button>
              </Row>
            </Row>
          </li>
        ))}
      </ul>
    </Panel>
  );
}