// StatusBar — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Versión simplificada (solo version + status + uptime)
// - 2026-09-01: `showDot` añade un punto pulsante de color al lado
//   del badge de estado para que el LLM-operador vea de un vistazo
//   si el daemon está conectado, degradado, o caído. La animación
//   `badge-pulse` ya existe en atoms.css (.badge--success/.warn/.info
//   .badge__dot). Para `error` el dot NO pulsa (rojo fijo) porque
//   las pulsaciones llamarían la atención sobre algo que ya es evidente.

import { Badge, type BadgeVariant } from "./atoms/Badge";
import { Row } from "./molecules/Row";

interface Props {
  version: string;
  status: "ok" | "degraded" | "error" | "connecting";
  uptimeSeconds?: number;
  /** Show the colored animated dot before the status badge. */
  showDot?: boolean;
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

export function StatusBar({ version, status, uptimeSeconds, showDot = false }: Props) {
  const { variant, label } = statusVariant[status];

  return (
    <Row gap="lg" align="center" data-testid="status-bar" data-status={status}>
      <span className="muted text-mono">v{version}</span>
      <Badge variant={variant} dot={showDot}>{label}</Badge>
      {uptimeSeconds !== undefined && (
        <span className="muted text-sm">↑ {formatUptime(uptimeSeconds)}</span>
      )}
    </Row>
  );
}