// DefaultAgentStatusPanel — display in-process default status. EP-0020-02.

import { useCallback, useEffect, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { StatPair } from "../shared/components/molecules/StatPair";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { getDefaultAgentStatus, type DefaultAgentResponse } from "../api/default";

export function DefaultAgentStatusPanel() {
  const [status, setStatus] = useState<DefaultAgentResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getDefaultAgentStatus();
      setStatus(data);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const interval = setInterval(load, 5000);
    return () => clearInterval(interval);
  }, [load]);

  if (loading && !status) return <p className="muted">Loading default status…</p>;
  if (!status) {
    return (
      <EmptyState>
        <EmptyState.Title>DefaultAgent unavailable</EmptyState.Title>
        <EmptyState.Hint>
          {error ?? "The in-process default is not initialised yet."}
        </EmptyState.Hint>
      </EmptyState>
    );
  }

  const { context, ...rest } = status;

  return (
    <Stack gap="md">
      <h3 className="models-section__title">In-process default</h3>
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card className="default-panel__card">
        <Row gap="md">
          <StatPair label="Tokens (est)" value={context.tokens_estimated} />
          <StatPair label="Messages" value={context.messages} />
          <StatPair label="Facts" value={rest.facts} />
          <StatPair label="Skills" value={rest.skills} />
        </Row>
        <Row gap="md">
          <StatPair label="Provider" value={rest.provider} />
          <StatPair label="Model" value={rest.model} />
          <StatPair
            label="Has summary"
            value={context.has_summary ? "yes" : "no"}
          />
          <StatPair
            label="Needs compaction"
            value={context.needs_compaction ? "yes" : "no"}
          />
        </Row>
      </Card>
    </Stack>
  );
}
