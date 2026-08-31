// ToolsPanel — list tools registered in the daemon. EP-0020-02.

import { useCallback, useEffect, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Badge } from "../shared/components/atoms/Badge";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { Code } from "../shared/components/atoms/Code";
import { getTools } from "../api/tools";
import type { ToolSpec } from "../types";

export function ToolsPanel() {
  const [tools, setTools] = useState<ToolSpec[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getTools();
      setTools(data.tools);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const toggle = (name: string) => {
    setExpanded((s) => {
      const next = new Set(s);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  if (loading) return <p className="muted">Loading tools…</p>;
  if (tools.length === 0) {
    return (
      <EmptyState>
        <EmptyState.Title>No tools registered</EmptyState.Title>
        <EmptyState.Hint>
          {error ?? "The daemon has no tools to expose."}
        </EmptyState.Hint>
      </EmptyState>
    );
  }

  return (
    <Stack gap="md">
      <h3 className="models-section__title">Tools ({tools.length})</h3>
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {tools.map((t) => (
        <Card key={t.name} className="tools-panel__card">
          <Row justify="between" align="center" className="tools-panel__header">
            <Row gap="md">
              <strong className="tools-panel__name">{t.name}</strong>
              <Badge variant={t.requires_approval ? "warn" : "neutral"}>
                {t.requires_approval ? "approval required" : "auto"}
              </Badge>
            </Row>
            <button
              type="button"
              className="tools-panel__toggle"
              onClick={() => toggle(t.name)}
              aria-expanded={expanded.has(t.name)}
            >
              {expanded.has(t.name) ? "▾ schema" : "▸ schema"}
            </button>
          </Row>
          <p className="tools-panel__description">{t.description}</p>
          {expanded.has(t.name) && (
            <Code className="tools-panel__json">
              {JSON.stringify(t.parameters, null, 2)}
            </Code>
          )}
        </Card>
      ))}
    </Stack>
  );
}
