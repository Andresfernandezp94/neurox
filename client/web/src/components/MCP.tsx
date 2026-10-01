// MCP — catálogo de tools expuestas por el motor de tools.
// Migrated to atomic design in EP-0016 (F4.5).
//
// OJO: este componente se llama "MCP" por historia, pero lo que muestra
// NO son servidores plugin registrados sino los specs de tools que
// devuelve `GET /v1/mcps` (`state.engine.tools.list_specs()`). La
// versión anterior pintaba `p.status`, `p.base_url`, `p.tools` y
// `p.skills`, campos que el daemon nunca envió: el status caía siempre en
// el badge "neutral" y la URL era cadena vacía. Tampoco existe
// `/v1/plugins` ni el campo `mcp_plugins` en el JSON, así que no hay
// forma de exponer los servidores plugin por HTTP: esa vista se elimina
// en vez de inventarla.
//
// `reconnectPlugin` se mantiene porque su endpoint sí existe, pero ya
// no hay a qué plugin reconectar desde acá: se expone como util y la
// reconexión se dispara desde donde se conoce el nombre real.

import { useCallback, useEffect, useState, type KeyboardEvent } from "react";
import { getPlugins, type McpToolSpec } from "../api/mcps";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Badge } from "../shared/components/atoms/Badge";
import { IconIntegrations } from "../shared/components/Icons";

export function MCP() {
  const [tools, setTools] = useState<McpToolSpec[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const fetchAll = useCallback(async () => {
    try {
      setTools(await getPlugins());
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

  const toggleExpanded = useCallback((name: string) => {
    setExpanded((prev) => ({ ...prev, [name]: !prev[name] }));
  }, []);

  return (
    <Stack gap="md" className="mcp-tab" data-testid="config-mcp-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {loading && <p className="muted">Loading...</p>}

      <h3 className="plugins-section__title">Tools ({tools.length})</h3>
      {!loading && tools.length === 0 && (
        <p className="muted">
          No tools exposed. The daemon publishes them from the tool engine
          registry; nothing to register from the web.
        </p>
      )}
      {tools.map((t) => {
        const isOpen = !!expanded[t.name];
        const params = Object.keys(t.parameters ?? {});
        return (
          <Card
            key={t.name}
            className={`plugin-card ${isOpen ? "plugin-card--expanded" : ""}`}
          >
            <div
              className="plugin-card__header"
              role="button"
              tabIndex={0}
              aria-expanded={isOpen}
              onClick={() => toggleExpanded(t.name)}
              onKeyDown={(e: KeyboardEvent<HTMLDivElement>) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  toggleExpanded(t.name);
                }
              }}
            >
              <Stack gap="sm">
                <Row
                  justify="between"
                  align="center"
                  className="plugin-card__title-row"
                >
                  <Row gap="sm" align="center" className="plugin-card__title-left">
                    <span className="plugin-card__status-dot" aria-hidden="true">
                      <IconIntegrations />
                    </span>
                    <strong className="plugin-card__name">{t.name}</strong>
                    {t.requires_approval && (
                      <Badge variant="warn">aprobación</Badge>
                    )}
                  </Row>
                  <Row gap="sm" align="center" className="plugin-card__actions">
                    {t.categories.map((c) => (
                      <span key={c} className="plugin-card__chip">
                        {c}
                      </span>
                    ))}
                  </Row>
                </Row>
                {t.description && (
                  <p className="muted plugin-card__url">{t.description}</p>
                )}
              </Stack>
            </div>

            {isOpen && (
              <div className="plugin-card__tools">
                <strong>Params ({params.length}):</strong>
                {params.length > 0 ? (
                  <ul className="tool-list">
                    {params.map((k) => (
                      <li key={k} className="tool-list__item">
                        <span className="tool-list__name" title={k}>
                          {k}
                        </span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <span className="muted">none</span>
                )}
                {t.mode_compatible.length > 0 && (
                  <p className="muted">
                    Modos: {t.mode_compatible.join(", ")}
                  </p>
                )}
              </div>
            )}
          </Card>
        );
      })}
    </Stack>
  );
}
