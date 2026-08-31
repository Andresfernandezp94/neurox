// StatusBar — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Versión simplificada (solo version + status + uptime)

import { Badge, type BadgeVariant } from "./atoms/Badge";
import { Row } from "./molecules/Row";

interface Props {
  version: string;
  status: "ok" | "degraded" | "error" | "connecting";
  uptimeSeconds?: number;
}

function formatUptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

const statusVariant: Record<
  Props["status"],
  { variant: BadgeVariant; label: string }
> = {
  ok: { variant: "success", label: "ok" },
  degraded: { variant: "warn", label: "degraded" },
  error: { variant: "danger", label: "error" },
  connecting: { variant: "info", label: "connecting…" },
};

export function StatusBar({ version, status, uptimeSeconds }: Props) {
  const { variant, label } = statusVariant[status];

  return (
    <Row gap="lg" align="center">
      <span className="muted text-mono">v{version}</span>
      <Badge variant={variant}>{label}</Badge>
      {uptimeSeconds !== undefined && (
        <span className="muted text-sm">↑ {formatUptime(uptimeSeconds)}</span>
      )}
    </Row>
  );
}