// ServicesPanel — list external services the daemon monitors. EP-0020-02.

import { useCallback, useEffect, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Badge, type BadgeVariant } from "../shared/components/atoms/Badge";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { getServices, type ServiceStatus } from "../api/services";

const statusVariant: Record<string, BadgeVariant> = {
  ok: "success",
  healthy: "success",
  connected: "success",
  warn: "warn",
  degraded: "warn",
  disconnected: "warn",
  error: "danger",
  failed: "danger",
};

export function ServicesTab() {
  const [services, setServices] = useState<ServiceStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getServices();
      setServices(data.services);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const interval = setInterval(load, 10000);
    return () => clearInterval(interval);
  }, [load]);

  if (loading) return <p className="muted">Loading services…</p>;
  if (services.length === 0) {
    return (
      <EmptyState>
        <EmptyState.Title>No services registered</EmptyState.Title>
        <EmptyState.Hint>
          {error ?? "The daemon has no external services to monitor."}
        </EmptyState.Hint>
      </EmptyState>
    );
  }

  return (
    <Stack gap="md" data-testid="config-services-tab">
      <h3 className="models-section__title">Services ({services.length})</h3>
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {services.map((s) => (
        <Card key={s.id} className="services-panel__card">
          <Row justify="between" align="center" className="services-panel__header">
            <Row gap="md">
              <strong className="services-panel__name">{s.name}</strong>
              <Badge variant={statusVariant[s.status] ?? "neutral"}>
                {s.status}
              </Badge>
              <code className="services-panel__kind">{s.kind}</code>
            </Row>
            <span className="muted">
              {s.latency_ms != null ? `${s.latency_ms}ms` : "—"}
            </span>
          </Row>
          <div className="muted services-panel__description">{s.description}</div>
          <div className="muted services-panel__endpoint">
            <code>{s.endpoint}</code>
          </div>
          {s.version && (
            <div className="muted services-panel__version">
              v{s.version}
            </div>
          )}
        </Card>
      ))}
    </Stack>
  );
}
