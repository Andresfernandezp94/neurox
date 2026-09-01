// Panel de Status — consume el store global + servicios externos. EP-0004.
// Migrated to atomic design in EP-0016 (F4.1): consumes <Card>, <Row>,
// <Stack>, <Badge> + utility classes instead of inline styles.
// EP-0024: tabs internas — "Overview" (default), "Host", "DefaultAgent".
// EP-0026-UX: el contenido de Overview absorbe lo que antes vivía en
// `ConfigViewer > GeneralTab` (runtime + services + plugins). El tab
// "General" de ConfigViewer se eliminó.
// cleanup-2026-08: los tabs "Host" (SystemMonitor), "Capabilities" y
// "Logs" se eliminaron junto con sus endpoints HTTP; solo queda Overview.
// 2026-09-01: el header "Runtime status" ahora renderiza un
// <StatusBar showDot /> con un dot pulsante que refleja la conexión
// con el daemon (ok=degraded=error=connecting). El usuario pidió un
// indicador tipo "luz" para ver de un vistazo si el daemon está vivo.

import { useCallback, useEffect, useState } from "react";
import { SectionHeader } from "../shared/components/SectionHeader";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { StatPair } from "../shared/components/molecules/StatPair";
import { Button } from "../shared/components/atoms/Button";
import { StatusBar } from "../shared/components/StatusBar";
import { useConnectionState } from "../store/StoreContext";
import { listServices, type ServiceInfo, type ClientInfo } from "../api/services";
import { getHealth } from "../api/health";
import { getPlugins, type PluginInfo } from "../api/mcps";
import type { Health } from "../types";
import {
  IconCpu,
  IconIntegrations,
  IconLoop,
  IconStatus,
} from "../shared/components/Icons";

export function StatusPanel() {
  const [services, setServices] = useState<ServiceInfo[]>([]);
  const [servicesError, setServicesError] = useState<string | null>(null);
  const [, setClients] = useState<ClientInfo[]>([]);
  const [serverTime, setServerTime] = useState<string | null>(null);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [healthInfo, setHealthInfo] = useState<Health | null>(null);
  const [loading, setLoading] = useState(true);

  // Fetch inicial. El refresh button llama a la misma función manualmente.
  // TODO: agregar polling con AbortController cuando lo pidan.
  const fetchAll = useCallback(async () => {
    try {
      const [h, svc, pls] = await Promise.all([
        getHealth(),
        listServices(),
        getPlugins(),
      ]);
      setHealthInfo(h);
      setServices(svc.services);
      setClients(svc.clients ?? []);
      setServerTime(svc.server_time);
      setPlugins(pls);
      setServicesError(null);
    } catch (e) {
      setServicesError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchAll();
  }, [fetchAll]);

  return (
    <Stack gap="md">
      <div className="page-pad">
        <OverviewContent
          health={healthInfo}
          services={services}
          serverTime={serverTime}
          plugins={plugins}
          loading={loading}
          error={servicesError}
          onRefresh={() => void fetchAll()}
        />
      </div>
    </Stack>
  );
}

/* -----------------------------------------------------------------------
 * Overview content — runtime + services + MCPs (absorbed de ConfigViewer
 * > GeneralTab en EP-0026-UX).
 * --------------------------------------------------------------------- */

interface OverviewContentProps {
  health: Health | null;
  services: ServiceInfo[];
  serverTime: string | null;
  plugins: PluginInfo[];
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

function OverviewContent({
  health,
  services,
  serverTime,
  plugins,
  loading,
  error,
  onRefresh,
}: OverviewContentProps) {
  const hasData = health !== null || services.length > 0 || plugins.length > 0;

  // DOT-fix: deriva el status de la status bar del estado de la
  // conexión WS. Si el daemon no responde, el último fetch dejó
  // `error` con un mensaje y marcamos "error" para encender el dot rojo.
  // Si todo OK, "ok". Si health llegó pero el WS está degradado,
  // "degraded". Si aún no sabemos nada, "connecting".
  const conn = useConnectionState();
  let barStatus: "ok" | "degraded" | "error" | "connecting";
  if (error) barStatus = "error";
  else if (conn.ws === "closed") barStatus = "error";
  else if (health === null) barStatus = "connecting";
  else if (conn.isZombie) barStatus = "degraded";
  else barStatus = "ok";

  return (
    <Stack gap="md" className="overview-content" data-testid="overview-content">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Row justify="between" align="center">
        <SectionHeader
          title="Overview"
          description={
            health
              ? "Runtime status, services and connected MCP plugins."
              : "Connecting to the daemon…"
          }
        />
        <Button
          variant="secondary"
          size="sm"
          onClick={onRefresh}
          disabled={loading}
          data-testid="overview-refresh"
        >
          <IconLoop /> Refresh
        </Button>
      </Row>

      {!hasData && !loading && !error && (
        <p className="muted">No data yet. Click Refresh.</p>
      )}

      {/* ─── Runtime status ─── */}
      <Row gap="sm" align="center" className="overview-content__section-header">
        <IconStatus />
        <h3 className="overview-content__section-title">Runtime status</h3>
      </Row>
      <Card className="overview-content__card">
        <Stack gap="sm">
          {/* DOT-fix: status bar con dot pulsante que muestra de un
              vistazo si el daemon está conectado. El color del dot
              sigue la variant del Badge (verde=ok, amarillo=degraded,
              rojo=error, azul=connecting). */}
          <StatusBar
            version={health?.version ?? "—"}
            status={barStatus}
            uptimeSeconds={health?.uptime_seconds}
            showDot
          />
          <StatPair label="Version" value={health?.version ?? "—"} />
          <StatPair label="Status" value={health?.status ?? "—"} />
          <StatPair label="Uptime" value={formatUptime(health?.uptime_seconds)} />
          <StatPair
            label="Started at"
            value={health?.started_at ? formatDate(health.started_at) : "—"}
          />
          <StatPair
            label="Auth"
            value={health?.auth_required ? "required" : "not required"}
          />
          <StatPair
            label="Server time"
            value={serverTime ? formatDate(serverTime) : "—"}
          />
        </Stack>
      </Card>

      {/* ─── Services ─── */}
      <Row gap="sm" align="center" className="overview-content__section-header">
        <IconCpu />
        <h3 className="overview-content__section-title">
          Services ({services.length})
        </h3>
      </Row>
      {services.length === 0 ? (
        <p className="muted">No services reported.</p>
      ) : (
        <Stack gap="sm">
          {services.map((s) => (
            <Card key={s.id} className="overview-content__card">
              <Row gap="sm" align="center">
                <span
                  className={`overview-content__item-icon overview-content__item-icon--${serviceStatusVariant(s.status)}`}
                  aria-hidden="true"
                >
                  <IconCpu />
                </span>
                <strong className="overview-content__service-name">{s.name}</strong>
                <span className="muted text-sm">· {s.kind}</span>
                {s.version && (
                  <span className="muted text-sm">· v{s.version}</span>
                )}
              </Row>
              {s.description && (
                <p className="muted overview-content__service-desc">
                  {s.description}
                </p>
              )}
              <Row gap="md">
                <StatPair label="Uptime" value={formatUptime(s.uptime_seconds)} />
                {s.latency_ms !== undefined && (
                  <StatPair label="Latency" value={`${s.latency_ms}ms`} />
                )}
              </Row>
            </Card>
          ))}
        </Stack>
      )}

      {/* ─── MCP plugins ─── */}
      <Row gap="sm" align="center" className="overview-content__section-header">
        <IconIntegrations />
        <h3 className="overview-content__section-title">
          MCP ({plugins.length})
        </h3>
      </Row>
      {plugins.length === 0 ? (
        <p className="muted">No plugins connected.</p>
      ) : (
        <Stack gap="sm">
          {plugins.map((p) => (
            <Card key={p.name} className="overview-content__card">
              <Row gap="sm" align="center">
                <span
                  className={`overview-content__item-icon overview-content__item-icon--${serviceStatusVariant(p.status)}`}
                  aria-hidden="true"
                >
                  <IconIntegrations />
                </span>
                <strong className="overview-content__service-name">{p.name}</strong>
                <span className="muted text-sm">· {p.tools?.length ?? 0} tools</span>
                <span className="muted text-sm">· {p.skills?.length ?? 0} skills</span>
              </Row>
              {p.base_url && (
                <p className="muted overview-content__service-desc">{p.base_url}</p>
              )}
            </Card>
          ))}
        </Stack>
      )}
    </Stack>
  );
}

/* -----------------------------------------------------------------------
 * Helpers
 * --------------------------------------------------------------------- */

function formatUptime(seconds?: number): string {
  if (!seconds && seconds !== 0) return "—";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

function serviceStatusVariant(
  status: string,
): "success" | "warn" | "danger" | "neutral" {
  switch (status) {
    case "ok":
    case "connected":
      return "success";
    case "degraded":
    case "disconnected":
      return "warn";
    case "error":
      return "danger";
    case "unreachable":
    case "unknown":
      return "neutral";
    default:
      return "neutral";
  }
}
