// OverviewPage — dashboard del panel interno (ruta /app).
//
// Qué es: la primera pantalla después de login. Responde "¿está todo
// bien?" y, sobre todo, "¿qué necesito hacer ahora?".
//
// Data — dos fuentes, deliberadamente separadas:
//
//   store    → agents, sessions, approvals, health, latencia. Ya los
//              mantiene el WS global; leerlos de ahí evita duplicar
//              requests y garantiza que el WS y la vista coincidan.
//   fetch    → providers, tools. El store NO los tiene, así que
//              se piden directo.
//
// Orden de la página: estado → lo que requiere atención → providers →
// detalle de runtime y tools. Lo accionable va arriba, no enterrado.

import { useCallback, useEffect, useState } from "react";
import { useStore, useConnectionState } from "../store/StoreContext";
import { Stack } from "../shared/components/molecules/Stack";
import { Row } from "../shared/components/molecules/Row";
import { Card } from "../shared/components/molecules/Card";
import { Badge, type BadgeVariant } from "../shared/components/atoms/Badge";
import { ConnectionStatus } from "../shared/components/ConnectionStatus";
import { SectionHeader } from "../shared/components/SectionHeader";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { ProviderLogo } from "../shared/components/ProviderLogo";
import {
  IconChat,
  IconClock,
  IconGrid,
  IconProviders,
  IconRobot,
} from "../shared/components/Icons";
import { getProviders, type LlmProviderStatus } from "../api/llm";
import { getTools } from "../api/tools";
import type { ToolSpec } from "../types";
import { providerDisplayName } from "../shared/providerLabels";

export function OverviewPage() {
  const { state } = useStore();
  const conn = useConnectionState();

  const [providers, setProviders] = useState<LlmProviderStatus[] | null>(null);
  const [tools, setTools] = useState<ToolSpec[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setError(null);
    try {
      // Promise.allSettled: que falle el catálogo no debe tumbar el
      // overview entero. Cada bloque se pinta por separado.
      const [provR, toolsR] = await Promise.allSettled([
        getProviders(),
        getTools(),
      ]);

      if (provR.status === "fulfilled") setProviders(provR.value.providers);
      if (toolsR.status === "fulfilled") setTools(toolsR.value.tools);

      const failed = [provR, toolsR].filter((r) => r.status === "rejected");
      if (failed.length === 2) {
        // Ambos fallaron: el daemon probablemente está caído o el token
        // expiró. Se da un mensaje accionable y se conserva la causa real,
        // que suele ser la que explica el problema.
        const reason = (failed[0] as PromiseRejectedResult).reason;
        const detail = reason instanceof Error ? reason.message : String(reason ?? "");
        setError(
          `No se pudo contactar el daemon${detail ? `: ${detail}` : ""}`,
        );
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // ─── Conteos (del store) ───
  const agents = state.agents.size;
  const sessions = state.sessions.size;
  const pendingApprovals = state.approvals.size;

  const configuredProviders =
    providers?.filter((p) => p.configured).length ?? null;
  // Categorías únicas del catálogo. Un tool sin categoría caería en un
  // grupo inexistente y desaparecería de la pantalla, así que se agrupa
  // bajo "sin categoría" explícito.
  const NO_CATEGORY = "sin categoría";
  const categories = Array.from(
    new Set(tools.flatMap((t) => t.categories?.filter(Boolean).length ? t.categories : [NO_CATEGORY])),
  ).sort();

  // ─── Cosas que requieren atención ───
  const attention: { kind: "warn" | "danger"; text: string }[] = [];
  if (pendingApprovals > 0) {
    attention.push({
      kind: "warn",
      text: `${pendingApprovals} aprobación${pendingApprovals === 1 ? "" : "es"} pendiente${pendingApprovals === 1 ? "" : "s"}`,
    });
  }
  if (conn.ws === "closed") {
    attention.push({ kind: "danger", text: "Sin conexión con el daemon" });
  }

  return (
    <Stack gap="lg" className="overview" data-testid="overview">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {/* ─── Encabezado ─── */}
      <Row justify="between" align="center">
        <SectionHeader
          title="Overview"
          description="Estado del daemon y lo que requiere atención."
        />
        <ConnectionStatus conn={conn} />
      </Row>

      {/* ─── Runtime ─── */}
      <section className="overview__section">
        <SectionHeader title="Runtime" />
        <Card>
          <div className="overview__runtime">
            <Stat label="Estado" value={conn.health?.status ?? "—"} />
            <Stat label="Versión" value={conn.health?.version ?? "—"} />
            <Stat label="Uptime" value={formatUptime(conn.health?.uptime_seconds)} />
            <Stat
              label="Inicio"
              value={
                conn.health?.started_at ? formatDate(conn.health.started_at) : "—"
              }
            />
            <Stat
              label="Auth"
              value={conn.health?.auth_required ? "requerida" : "no requerida"}
            />
          </div>
        </Card>
      </section>

      {/* ─── Stats ─── */}
      <div className="overview__stats">
        <StatCard label="Agentes" value={agents} icon={<IconRobot />} />
        <StatCard label="Sesiones" value={sessions} icon={<IconChat />} />
        {/* No hay tarjeta de "Tools": su total ya aparece como hint en
            Categorías, y mantener las dos era redundante. */}
        <StatCard
          label="Categorías"
          value={tools.length === 0 && loading ? "—" : categories.length}
          hint={tools.length ? `${tools.length} tools` : undefined}
          icon={<IconGrid />}
        />
        <StatCard
          label="Providers"
          value={configuredProviders ?? "—"}
          hint={
            providers
              ? `${configuredProviders} de ${providers.length} con key`
              : undefined
          }
          icon={<IconProviders />}
        />
        <StatCard
          label="Latencia"
          value={conn.latencyMs !== null ? `${conn.latencyMs}ms` : "—"}
          icon={<IconClock />}
        />
      </div>

      {/* ─── Requiere atención ─── */}
      {attention.length > 0 && (
        <section className="overview__section">
          <SectionHeader title="Requiere atención" />
          <Stack gap="sm">
            {attention.map((a) => (
              <Card key={a.text} className="overview__attention">
                <span
                  className={`overview__attention-dot overview__attention-dot--${a.kind}`}
                  aria-hidden="true"
                />
                <span>{a.text}</span>
              </Card>
            ))}
          </Stack>
        </section>
      )}

      {/* ─── Providers ─── */}
      <section className="overview__section">
        <SectionHeader
          title="Providers LLM"
          description="Providers con credencial configurada."
        />
        {providers === null ? (
          loading ? (
            <p className="muted">Cargando providers…</p>
          ) : (
            <EmptyState>
              <EmptyState.Title>No se pudieron leer los providers.</EmptyState.Title>
            </EmptyState>
          )
        ) : providers.length === 0 ? (
          <EmptyState>
            <EmptyState.Title>Ningún provider registrado.</EmptyState.Title>
            <EmptyState.Hint>Registralos desde Configuración.</EmptyState.Hint>
          </EmptyState>
        ) : (
          <div className="overview__providers">
            {providers.map((p) => (
              <Card key={p.id} className="overview__provider">
                <Row gap="sm" align="center" justify="between">
                  <Row gap="sm" align="center">
                    <ProviderLogo id={p.id} kind={p.kind} />
                    <strong className="overview__provider-name">
                    {providerDisplayName(p.id, p.kind)}
                  </strong>
                  </Row>
                  {p.configured && p.active ? (
                    // El pill accent solido es el que ya usan ProvidersPanel,
                    // ModelsTab, UsersPanel y EnvTab para marcar lo activo.
                    // `variant="success"` era un verde generico que no
                    // comunicaba "este es el que se esta usando".
                    <Badge className="badge--active">ACTIVE</Badge>
                  ) : (
                    <Badge variant={providerVariant(p)}>{providerLabel(p)}</Badge>
                  )}
                </Row>
                <p className="overview__provider-model muted text-sm">
                  {p.model}
                </p>
              </Card>
            ))}
          </div>
        )}
      </section>

      {/* ─── Catálogo de tools ─── */}
      <section className="overview__section">
        <SectionHeader
          title="Tools publicadas"
          description={
            tools.length
              ? `${tools.length} tools en ${categories.length} categorías`
              : undefined
          }
        />
        {tools.length === 0 ? (
          loading ? (
            <p className="muted">Cargando tools…</p>
          ) : (
            <EmptyState>
              <EmptyState.Title>No hay tools registradas.</EmptyState.Title>
              <EmptyState.Hint>
                El motor expone las tools nativas. Registralas en
                <code> config.yaml </code> o reiniciá el daemon.
              </EmptyState.Hint>
            </EmptyState>
          )
        ) : (
          <Stack gap="md">
            {categories.map((cat) => (
              <div key={cat} className="overview__category">
                <span className="overview__category-name">{cat}</span>
                <div className="overview__tool-list">
                  {tools
                    .filter((t) => {
                      const cats = t.categories?.filter(Boolean) ?? [];
                      return cats.length ? cats.includes(cat) : cat === NO_CATEGORY;
                    })
                    .map((t) => (
                      <div key={t.name} className="overview__tool">
                        <Row gap="sm" align="center" justify="between">
                          <span className="overview__tool-name">{t.name}</span>
                          {t.requires_approval && (
                            <Badge variant="warn">requiere aprobación</Badge>
                          )}
                        </Row>
                        <p className="overview__tool-desc muted text-sm">
                          {t.description}
                        </p>
                      </div>
                    ))}
                </div>
              </div>
            ))}
          </Stack>
        )}
      </section>
    </Stack>
  );
}

/* ── Subcomponentes ─────────────────────────────────────────── */

function StatCard({
  label,
  value,
  hint,
  icon,
}: {
  label: string;
  value: number | string;
  hint?: string;
  icon?: React.ReactNode;
}) {
  return (
    <Card className="overview__stat">
      <Row gap="sm" align="center" className="overview__stat-head">
        {icon && <span className="overview__stat-icon">{icon}</span>}
        <span className="overview__stat-label">{label}</span>
      </Row>
      <span className="overview__stat-value">{value}</span>
      {hint && <span className="overview__stat-hint muted">{hint}</span>}
    </Card>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="overview__runtime-item">
      <span className="overview__stat-label">{label}</span>
      <span className="overview__runtime-value">{value}</span>
    </div>
  );
}

/* ── Helpers ───────────────────────────────────────────────── */

/// Variante para el provider que NO es el activo: el activo se resuelve
/// aparte porque usa el pill accent solido y no una variante de color.
function providerVariant(p: LlmProviderStatus): BadgeVariant {
  if (p.configured) return "info";
  return "neutral";
}

/// Texto del badge para los providers que no son el activo.
function providerLabel(p: LlmProviderStatus): string {
  if (p.configured) return "configurado";
  return "sin key";
}

function formatUptime(seconds?: number): string {
  if (seconds === undefined || seconds === null) return "—";
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${seconds}s`;
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}