// Plugins Panel — shows connected plugins, the installable catalog, and
// reconnect/install actions. Migrated to atomic design in EP-0016 (F4.5)
// and extended in EP-0021 to surface the operator-facing catalog.

import { useCallback, useEffect, useState, type KeyboardEvent } from "react";
import {
  getPlugins,
  reconnectPlugin,
  type PluginInfo,
} from "../api/mcps";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { type BadgeVariant } from "../shared/components/atoms/Badge";
import { Button } from "../shared/components/atoms/Button";
import { IconLoop, IconConfig } from "../shared/components/Icons";

const statusVariant: Record<string, BadgeVariant> = {
  connected: "success",
  disconnected: "warn",
  error: "danger",
};

export function MCP() {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const fetchAll = useCallback(async () => {
    try {
      const live = await getPlugins();
      setPlugins(live);
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchAll();
    const interval = setInterval(fetchAll, 5000);
    return () => clearInterval(interval);
  }, [fetchAll]);

  const handleReconnect = useCallback(
    async (name: string) => {
      setBusy(name);
      setError(null);
      try {
        await reconnectPlugin(name);
        await fetchAll();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setBusy(null);
      }
    },
    [fetchAll],
  );

  const toggleExpanded = useCallback((name: string) => {
    setExpanded((prev) => ({ ...prev, [name]: !prev[name] }));
  }, []);

  return (
    <Stack gap="md" className="mcp-tab" data-testid="config-mcp-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {loading && <p className="muted">Loading...</p>}

      <h3 className="plugins-section__title">Installed ({plugins.length})</h3>
      {!loading && plugins.length === 0 && (
        <p className="muted">No plugins registered. The daemon will pick up new plugins automatically when they register via POST /v1/mcps.</p>
      )}
      {plugins.map((p) => {
        const isOpen = !!expanded[p.name];
        return (
        <Card key={p.name} className={`plugin-card ${isOpen ? "plugin-card--expanded" : ""}`}>
          <div
            className="plugin-card__header"
            role="button"
            tabIndex={0}
            aria-expanded={isOpen}
            onClick={() => toggleExpanded(p.name)}
            onKeyDown={(e: KeyboardEvent<HTMLDivElement>) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                toggleExpanded(p.name);
              }
            }}
          >
          <Stack gap="sm">
            <Row justify="between" align="center" className="plugin-card__title-row">
              <Row gap="sm" align="center" className="plugin-card__title-left">
                <span
                  className={`plugin-card__status-dot plugin-card__status-dot--${statusVariant[p.status] ?? "neutral"}`}
                  title={p.status}
                  aria-label={p.status}
                />
                <strong className="plugin-card__name">{p.name}</strong>
              </Row>
              <Row
                gap="sm"
                align="center"
                className="plugin-card__actions"
                onClick={(e) => e.stopPropagation()}
              >
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={busy === p.name}
                  onClick={() => void handleReconnect(p.name)}
                  title="Reconnect plugin (2 retries)"
                  aria-label="Reconnect plugin"
                >
                  {busy === p.name ? "..." : <IconLoop />}
                </Button>
                <span className="plugin-card__divider-v" aria-hidden="true">
                  |
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    // TODO: abrir modal/página de configuración del MCP
                  }}
                  title="Configuración del MCP"
                  aria-label="Configuración del MCP"
                >
                  <IconConfig />
                </Button>
              </Row>
            </Row>
            <Row align="center" className="plugin-card__header-info">
              <span
                className="muted plugin-card__url"
                title={p.base_url}
              >
                {p.base_url}
              </span>
            </Row>
          </Stack>
          </div>

          {isOpen && (
            <>
              <div className="plugin-card__tools">
                <strong>Tools ({p.tools.length}):</strong>
                {p.tools.length > 0 ? (
                  <ul className="tool-list">
                    {p.tools.map((t) => (
                      <li key={t} className="tool-list__item">
                        <span className="tool-list__name" title={t}>
                          {t}
                        </span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <span className="muted">none</span>
                )}
              </div>

              {p.skills.length > 0 && (
                <div className="plugin-card__skills">
                  <strong>Skills ({p.skills.length}):</strong>{" "}
                  {p.skills.map((s) => (
                    <code key={s} className="plugin-card__chip">
                      {s}
                    </code>
                  ))}
                </div>
              )}
            </>
          )}
        </Card>
        );
      })}
    </Stack>
  );
}
