// EnvTab — list/set env vars via /v1/env. EP-0020-02.
// Convention: values are NEVER returned by the backend (security).
// Migrado al look and feel de MCP/General (sesión 2026-08-14):
//   - Container con padding y gap consistente
//   - Header con título + Refresh button (icon)
//   - Cards por env var (no tabla)
//   - Iconos en section header + key icon + Save button
//   - Env vars agrupadas en categorías (daemon / web / mcp) con toggle

import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Input } from "../shared/components/atoms/Input";
import { IconButton } from "../shared/components/atoms/IconButton";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import {
  IconCode,
  IconCpu,
  IconGlobe,
} from "../shared/components/Icons";
import { getEnv, putEnvVar, deleteEnvVar, type EnvVar } from "../api/env";

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
    k.startsWith("MCP_")
  ) {
    return "mcp";
  }
  return "daemon";
}

export function EnvTab() {
  const [vars, setVars] = useState<EnvVar[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getEnv();
      setVars(data.vars);
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

  const handleClear = useCallback(
    async (key: string) => {
      setPending(key);
      setError(null);
      try {
        await deleteEnvVar(key);
        await load();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setPending(null);
      }
    },
    [load],
  );

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

  return (
    <div className="providers-list" data-testid="config-env-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <p className="muted text-sm providers-panel__description">
        Environment variables live in the daemon's env file — values are never
        returned for security.
      </p>

      {loading ? (
        <p className="muted">Loading env vars…</p>
      ) : vars.length === 0 ? (
        <EmptyState>
          <EmptyState.Title>No env vars configured</EmptyState.Title>
          <EmptyState.Hint>{error ?? "The env file is empty."}</EmptyState.Hint>
        </EmptyState>
      ) : (
        CATEGORY_ORDER.map((cat) => {
          const items = grouped[cat];
          if (items.length === 0) return null;
          const meta = CATEGORY_META[cat];
          return (
            <Fragment key={cat}>
              <div className="providers-list__section-head">
                <h4 className="muted">
                  <span className="env-tab__category-icon">{meta.icon}</span>
                  {meta.label}
                </h4>
                <span className="muted text-sm">{items.length}</span>
              </div>
              {items.map((v) => (
                <Card key={v.key} className="provider-card">
                  <Row justify="between" align="center" gap="sm">
                    <strong className="strong">{v.key}</strong>
                    <span
                      className={`env-tab__state-badge env-tab__state-badge--${v.set ? "set" : "unset"}`}
                    >
                      {v.set ? "set" : "unset"}
                    </span>
                  </Row>
                  <Row gap="sm" align="stretch" className="provider-keyrow">
                    <Input
                      type="password"
                      placeholder={v.set ? "(set)" : "(unset)"}
                      value={drafts[v.key] ?? ""}
                      onChange={(e) =>
                        setDrafts((d) => ({ ...d, [v.key]: e.target.value }))
                      }
                      className="env-tab__input"
                    />
                    <IconButton
                      icon="IconSave"
                      aria-label={`Save ${v.key}`}
                      title="Save"
                      disabled={pending === v.key || drafts[v.key] === undefined}
                      onClick={() => void handleSave(v.key)}
                    />
                    {v.set && (
                      <IconButton
                        icon="IconTrash"
                        aria-label={`Clear ${v.key}`}
                        title="Clear"
                        variant="danger"
                        disabled={pending === v.key}
                        onClick={() => void handleClear(v.key)}
                      />
                    )}
                  </Row>
                </Card>
              ))}
            </Fragment>
          );
        })
      )}
    </div>
  );
}
