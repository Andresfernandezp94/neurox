// EnvTab — list/set env vars via /v1/env. EP-0020-02.
// Convention: values are NEVER returned by the backend (security).
// Migrado al look and feel de MCP/General (sesión 2026-08-14):
//   - Container con padding y gap consistente
//   - Header con título + Refresh button (icon)
//   - Cards por env var (no tabla)
//   - Iconos en section header + key icon + Save button
//   - Env vars agrupadas en categorías (daemon / web / mcp) con toggle

import { useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { Input } from "../shared/components/atoms/Input";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import {
  IconCode,
  IconCpu,
  IconGlobe,
  IconLoop,
  IconSave,
} from "../shared/components/Icons";
import { getEnv, putEnvVar, type EnvVar } from "../api/env";

type Category = "daemon" | "web" | "mcp";

const CATEGORY_META: Record<Category, { label: string; icon: React.ReactNode }> =
  {
    daemon: { label: "Daemon", icon: <IconCpu /> },
    web: { label: "Web", icon: <IconGlobe /> },
    mcp: { label: "MCP", icon: <IconCode /> },
  };

const CATEGORY_ORDER: Category[] = ["daemon", "web", "mcp"];

function categorizeKey(key: string): Category {
  const k = key.toUpperCase();
  if (
    k.startsWith("VITE_") ||
    k.startsWith("WEB_") ||
    k.startsWith("CF_") ||
    k.startsWith("CLOUDFLARE_")
  ) {
    return "web";
  }
  if (
    k.startsWith("MEMORY_") ||
    k.startsWith("VOICE_") ||
    k.startsWith("LLMD_") ||
    k.startsWith("CLICKUP_") ||
    k.startsWith("MCP_") ||
    k.startsWith("NEUROX_")
  ) {
    return "mcp";
  }
  return "daemon";
}

export function EnvTab() {
  const [vars, setVars] = useState<EnvVar[]>([]);
  const [path, setPath] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [expanded, setExpanded] = useState<Record<Category, boolean>>({
    daemon: false,
    web: false,
    mcp: false,
  });

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getEnv();
      setVars(data.vars);
      setPath(data.path);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleSave = useCallback(
    async (key: string) => {
      const value = drafts[key];
      if (value === undefined) return;
      setPending(key);
      setError(null);
      try {
        await putEnvVar(key, value);
        setDrafts((d) => {
          const next = { ...d };
          delete next[key];
          return next;
        });
        await load();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setPending(null);
      }
    },
    [drafts, load],
  );

  const toggleCategory = useCallback((cat: Category) => {
    setExpanded((prev) => ({ ...prev, [cat]: !prev[cat] }));
  }, []);

  // Agrupa las env vars por categoría. Se recalcula solo cuando cambian vars.
  const grouped = useMemo(() => {
    const result: Record<Category, EnvVar[]> = {
      daemon: [],
      web: [],
      mcp: [],
    };
    for (const v of vars) {
      result[categorizeKey(v.key)].push(v);
    }
    return result;
  }, [vars]);

  const totalCount = vars.length;

  return (
    <Stack gap="md" className="env-tab" data-testid="config-env-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Row justify="between" align="center">
        <h2 className="strong text-lg env-tab__title">Environments</h2>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => void load()}
          disabled={loading}
        >
          <IconLoop /> Refresh
        </Button>
      </Row>

      {!loading && vars.length > 0 && (
        <p className="muted env-tab__subtitle">
          File: <code className="env-tab__path">{path}</code> ·{" "}
          {totalCount} vars
        </p>
      )}

      {loading ? (
        <p className="muted">Loading env vars…</p>
      ) : vars.length === 0 ? (
        <EmptyState>
          <EmptyState.Title>No env vars configured</EmptyState.Title>
          <EmptyState.Hint>{error ?? "The env file is empty."}</EmptyState.Hint>
        </EmptyState>
      ) : (
        <Stack gap="sm">
          {CATEGORY_ORDER.map((cat) => {
            const items = grouped[cat];
            if (items.length === 0) return null;
            const meta = CATEGORY_META[cat];
            const isOpen = expanded[cat];
            return (
              <Card
                key={cat}
                className={`env-tab__category ${isOpen ? "env-tab__category--expanded" : ""}`}
              >
                <div
                  className="env-tab__category-header"
                  role="button"
                  tabIndex={0}
                  aria-expanded={isOpen}
                  onClick={() => toggleCategory(cat)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      toggleCategory(cat);
                    }
                  }}
                >
                  <Row gap="sm" align="center">
                    <span className="env-tab__category-icon" aria-hidden="true">
                      {meta.icon}
                    </span>
                    <strong className="env-tab__category-label">
                      {meta.label}
                    </strong>
                    <span className="env-tab__category-count">
                      ({items.length})
                    </span>
                  </Row>
                  <span
                    className={`env-tab__chevron ${isOpen ? "env-tab__chevron--open" : ""}`}
                    aria-hidden="true"
                  >
                    ▾
                  </span>
                </div>

                {isOpen && (
                  <Stack gap="sm" className="env-tab__category-body">
                    {items.map((v) => (
                      <div key={v.key} className="env-tab__row">
                        <Row justify="between" align="center" gap="sm">
                          <Row gap="sm" align="center" className="env-tab__key">
                            <code className="env-tab__key-name">{v.key}</code>
                            <span
                              className={`env-tab__state-badge env-tab__state-badge--${v.set ? "set" : "unset"}`}
                            >
                              {v.set ? "set" : "unset"}
                            </span>
                          </Row>
                          <Row gap="sm" align="center">
                            <Input
                              type="password"
                              placeholder={v.set ? "(set)" : "(unset)"}
                              value={drafts[v.key] ?? ""}
                              onChange={(e) =>
                                setDrafts((d) => ({
                                  ...d,
                                  [v.key]: e.target.value,
                                }))
                              }
                              className="env-tab__input"
                            />
                            <Button
                              variant="secondary"
                              size="sm"
                              disabled={
                                pending === v.key || drafts[v.key] === undefined
                              }
                              onClick={() => handleSave(v.key)}
                            >
                              <IconSave /> {pending === v.key ? "…" : "Save"}
                            </Button>
                          </Row>
                        </Row>
                      </div>
                    ))}
                  </Stack>
                )}
              </Card>
            );
          })}
        </Stack>
      )}
    </Stack>
  );
}
