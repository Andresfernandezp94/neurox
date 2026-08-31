// Panel de Live Events — WebSocket stream filtrable + pause + clear.
// EP-0001-02 (migrado a tokens agent-studio en EP-0002-01 Task 11)
// EP-0003-01: muestra latencia y estado zombie del heartbeat.
// SectionHeader viene de App.tsx. Mantiene toolbar local (status, pause, clear).
// Migrated to atomic design in EP-0016 (F4.3).

import { useMemo, useState } from "react";
import { useWebSocket } from "../hooks/useWebSocket";
import { useWsHeartbeat } from "../hooks/useWsHeartbeat";
import type { DaemonEvent } from "../types";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { Card } from "../shared/components/molecules/Card";
import { Badge, type BadgeVariant } from "../shared/components/atoms/Badge";
import { Button } from "../shared/components/atoms/Button";

const TYPE_FILTERS = ["agent", "session", "tool", "approval", "system", "other"] as const;

function eventType(e: DaemonEvent): string {
  const t = typeof e.type === "string" ? e.type : "";
  const lower = t.toLowerCase();
  for (const f of TYPE_FILTERS) {
    if (lower.includes(f)) return f;
  }
  return "other";
}

const filterVariant: Record<string, BadgeVariant> = {
  agent: "success",
  session: "info",
  tool: "info",
  approval: "warn",
  system: "neutral",
  other: "neutral",
};

function statusVariant(
  isZombie: boolean,
  status: string,
): BadgeVariant {
  if (isZombie) return "danger";
  if (status === "open") return "success";
  if (status === "connecting") return "warn";
  return "danger";
}

export function LiveEventsPanel() {
  const { events, status, clear } = useWebSocket("/v1/events");
  const { isZombie, latencyMs } = useWsHeartbeat();
  const [filters, setFilters] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(TYPE_FILTERS.map((t) => [t, true])),
  );
  const [paused, setPaused] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);

  const statusLabel = isZombie ? `${status} (zombie)` : status;
  const variant = statusVariant(isZombie, status);

  const visible = useMemo(
    () => events.filter((e) => filters[eventType(e)] ?? false),
    [events, filters],
  );

  const toggleFilter = (t: string) => setFilters((f) => ({ ...f, [t]: !f[t] }));

  return (
    <Stack gap="md">
      <Row gap="sm" wrap>
        <Badge variant={variant}>{statusLabel}</Badge>
        {latencyMs !== null && status === "open" && !isZombie && (
          <Badge
            variant={latencyMs > 1000 ? "warn" : "neutral"}
            className="text-mono"
          >
            {latencyMs}ms
          </Badge>
        )}
        <Button
          size="sm"
          variant={paused ? "danger" : "secondary"}
          onClick={() => setPaused((p) => !p)}
        >
          {paused ? "Resume" : "Pause"}
        </Button>
        <Button
          size="sm"
          variant={autoScroll ? "primary" : "secondary"}
          onClick={() => setAutoScroll((a) => !a)}
        >
          Auto-scroll
        </Button>
        <Button size="sm" variant="secondary" onClick={clear}>
          Clear
        </Button>
      </Row>

      <Row gap="sm" wrap>
        {TYPE_FILTERS.map((t) => (
          <label key={t} className="live-events__filter">
            <input
              type="checkbox"
              checked={filters[t] ?? false}
              onChange={() => toggleFilter(t)}
            />
            <Badge variant={filterVariant[t] ?? "neutral"}>{t}</Badge>
          </label>
        ))}
      </Row>

      <Card gap="none" className="live-events__stream">
        {visible.length === 0 ? (
          <EmptyState>
            <EmptyState.Hint>Waiting for events…</EmptyState.Hint>
          </EmptyState>
        ) : (
          visible.map((e, i) => {
            const t = eventType(e);
            const ts =
              typeof e.ts === "string"
                ? e.ts
                : typeof e.timestamp === "string"
                ? e.timestamp
                : "";
            return (
              <Row key={i} gap="sm" align="start" className="live-events__row">
                <span className="muted live-events__ts">{ts}</span>
                <Badge variant={filterVariant[t] ?? "neutral"}>{t}</Badge>
                <span className="live-events__payload">
                  {JSON.stringify(e, null, 0)}
                </span>
              </Row>
            );
          })
        )}
        {paused && visible.length > 0 && (
          <div className="live-events__paused">
            (paused — auto-scroll disabled)
          </div>
        )}
      </Card>
    </Stack>
  );
}