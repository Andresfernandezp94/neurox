// SandboxTab — runtime-patch the daemon's sandbox config via PUT /v1/sandbox.
//
// EP-0024-UX: reorganised into collapsible sections (General /
// writable_paths / readable_paths / Effective scope) and a path
// chip editor so the operator doesn't have to scroll through a
// wall of text + textarea. ${workspace} resolution is shown inline
// on each chip (and on hover for tooltip).
//
// Surfaces every SandboxConfig field the backend exposes:
//   - enabled (bool)
//   - writable_paths (list; read+write)
//   - readable_paths (list; read-only)
//   - max_recursion_depth (usize)

import { useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { CollapsibleSection } from "../shared/components/molecules/CollapsibleSection";
import { PathChipsEditor, resolvePath } from "../shared/components/molecules/PathChipsEditor";
import { putSandbox, type SandboxConfig } from "../api/sandbox";
import { getDefaultAgentStatus, type DefaultAgentResponse } from "../api/default";

const MIN_DEPTH = 1;
const MAX_DEPTH = 100;

function depthBadge(depth: number): string {
  return `${depth}`;
}

export function SandboxTab() {
  const [config, setConfig] = useState<SandboxConfig | null>(null);
  const [draft, setDraft] = useState<SandboxConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [defaultAgent, setDefaultAgent] = useState<DefaultAgentResponse | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [data, status] = await Promise.all([
        putSandbox({}), // empty patch returns current config
        getDefaultAgentStatus().catch(() => null),
      ]);
      setConfig(data);
      setDraft(data);
      if (status) setDefaultAgent(status);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Refresh default status in the background so the workspace preview
  // stays current without a full reload.
  useEffect(() => {
    const poll = async () => {
      try {
        const status = await getDefaultAgentStatus();
        setDefaultAgent(status);
      } catch {
        /* ignore — default is best-effort here */
      }
    };
    const interval = setInterval(() => void poll(), 10_000);
    return () => clearInterval(interval);
  }, []);

  const handleSave = useCallback(async () => {
    if (!draft) return;
    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      const updated = await putSandbox(draft);
      setConfig(updated);
      setDraft(updated);
      setSaved(true);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  }, [draft]);

  const workspace = defaultAgent?.cwd ?? "";

  const dirty = useMemo(() => {
    if (!config || !draft) return false;
    return (
      config.enabled !== draft.enabled ||
      config.max_recursion_depth !== draft.max_recursion_depth ||
      JSON.stringify(config.writable_paths) !==
        JSON.stringify(draft.writable_paths) ||
      JSON.stringify(config.readable_paths) !==
        JSON.stringify(draft.readable_paths)
    );
  }, [config, draft]);

  if (loading) return <p className="muted">Loading sandbox config…</p>;
  if (!draft) {
    return (
      <EmptyState>
        <EmptyState.Title>Sandbox config unavailable</EmptyState.Title>
        <EmptyState.Hint>{error ?? "No data."}</EmptyState.Hint>
      </EmptyState>
    );
  }

  // Effective read scope = workspace root + all readable paths + all
  // writable paths (writes imply reads). De-duped.
  const effectiveRead = Array.from(
    new Set(
      [
        ...(workspace ? [workspace] : []),
        ...draft.readable_paths.map((p) => resolvePath(p, workspace)),
        ...draft.writable_paths.map((p) => resolvePath(p, workspace)),
      ].filter((p) => p.length > 0),
    ),
  );
  const effectiveWrite = Array.from(
    new Set(draft.writable_paths.map((p) => resolvePath(p, workspace))),
  );

  return (
    <Stack gap="md" data-testid="config-sandbox-tab">
      <header className="sandbox-panel__header">
        <div>
          <h3 className="sandbox-panel__title">Sandbox</h3>
          <p className="sandbox-panel__subtitle">
            Runtime-patch the daemon's sandbox. Changes apply on Apply.
          </p>
        </div>
      </header>

      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card className="sandbox-panel__card">
        {/* ─── General: enabled + max_recursion_depth side-by-side ─── */}
        <div className="sandbox-panel__general">
          <div className="sandbox-panel__field">
            <div className="sandbox-panel__field-label">
              <span>Enabled</span>
              <label className="sandbox-panel__switch">
                <input
                  type="checkbox"
                  data-testid="sandbox-enabled"
                  checked={draft.enabled}
                  onChange={(e) =>
                    setDraft({ ...draft, enabled: e.target.checked })
                  }
                />
                <span className="sandbox-panel__switch-track" />
              </label>
            </div>
            <p className="sandbox-panel__field-hint">
              When off, every tool call bypasses the path checks below.
            </p>
          </div>

          <div className="sandbox-panel__field">
            <div className="sandbox-panel__field-label">
              <span>Max recursion depth</span>
              <div className="sandbox-panel__stepper">
                <button
                  type="button"
                  className="sandbox-panel__stepper-btn"
                  data-testid="sandbox-depth-dec"
                  aria-label="Decrease depth"
                  onClick={() =>
                    setDraft({
                      ...draft,
                      max_recursion_depth: Math.max(
                        MIN_DEPTH,
                        draft.max_recursion_depth - 1,
                      ),
                    })
                  }
                >
                  −
                </button>
                <input
                  type="number"
                  className="sandbox-panel__stepper-input"
                  data-testid="sandbox-depth"
                  min={MIN_DEPTH}
                  max={MAX_DEPTH}
                  value={draft.max_recursion_depth}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      max_recursion_depth: Math.max(
                        MIN_DEPTH,
                        Math.min(
                          MAX_DEPTH,
                          parseInt(e.target.value, 10) || MIN_DEPTH,
                        ),
                      ),
                    })
                  }
                />
                <button
                  type="button"
                  className="sandbox-panel__stepper-btn"
                  data-testid="sandbox-depth-inc"
                  aria-label="Increase depth"
                  onClick={() =>
                    setDraft({
                      ...draft,
                      max_recursion_depth: Math.min(
                        MAX_DEPTH,
                        draft.max_recursion_depth + 1,
                      ),
                    })
                  }
                >
                  +
                </button>
              </div>
            </div>
            <p className="sandbox-panel__field-hint">
              Cap on how deep <code>glob</code> / <code>grep</code> descend
              ({MIN_DEPTH}–{MAX_DEPTH}).
            </p>
          </div>
        </div>

        {/* ─── Writable paths ─── */}
        <CollapsibleSection
          title="Writable paths"
          badge={draft.writable_paths.length}
          badgeClassName={
            draft.writable_paths.length > 0 ? "collapsible__badge--accent" : undefined
          }
          hint="paths where write tools are allowed"
          data-testid="sandbox-writable-section"
        >
          <PathChipsEditor
            paths={draft.writable_paths}
            onChange={(next) =>
              setDraft({ ...draft, writable_paths: next })
            }
            workspaceRoot={workspace}
            placeholder="${workspace}/.sdd"
            testIdPrefix="sandbox-writable-paths"
          />
        </CollapsibleSection>

        {/* ─── Readable paths ─── */}
        <CollapsibleSection
          title="Readable paths"
          badge={draft.readable_paths.length}
          badgeClassName={
            draft.readable_paths.length > 0 ? "collapsible__badge--accent" : undefined
          }
          hint="paths where read tools are allowed (writes rejected)"
          data-testid="sandbox-readable-section"
        >
          <PathChipsEditor
            paths={draft.readable_paths}
            onChange={(next) =>
              setDraft({ ...draft, readable_paths: next })
            }
            workspaceRoot={workspace}
            placeholder="${workspace}/docs"
            testIdPrefix="sandbox-readable-paths"
          />
        </CollapsibleSection>

        <div className="sandbox-panel__footer">
          {dirty ? (
            <span className="sandbox-panel__dirty" data-testid="sandbox-dirty">
              ● unsaved changes
            </span>
          ) : saved ? (
            <span className="sandbox-panel__saved" data-testid="sandbox-saved">
              ✓ saved
            </span>
          ) : (
            <span />
          )}
          <Button
            variant="primary"
            data-testid="sandbox-apply"
            disabled={!dirty || saving}
            onClick={handleSave}
          >
            {saving ? "Saving…" : "Apply"}
          </Button>
        </div>
      </Card>

      {/* ─── Effective scope (read-only) ─── */}
      <Card className="sandbox-panel__card sandbox-panel__effective">
        <CollapsibleSection
          title="Effective scope"
          defaultOpen={false}
          data-testid="sandbox-effective"
        >
          <dl className="sandbox-panel__kv">
            <dt>workspace_root</dt>
            <dd data-testid="sandbox-workspace-root">
              {workspace || <span className="muted">not reported</span>}
            </dd>
            <dt>git_branch</dt>
            <dd>
              {defaultAgent?.git_branch ?? (
                <span className="muted">not a git repo / detached HEAD</span>
              )}
            </dd>
            <dt>sandbox.enabled</dt>
            <dd>{draft.enabled ? "true" : "false"}</dd>
            <dt>max_recursion_depth</dt>
            <dd>{depthBadge(draft.max_recursion_depth)}</dd>
            <dt>effective write paths ({effectiveWrite.length})</dt>
            <dd>
              {effectiveWrite.length === 0 ? (
                <span className="muted">none — all writes rejected</span>
              ) : (
                effectiveWrite.map((p, i) => (
                  <div key={i}>
                    <code>{p}</code>
                  </div>
                ))
              )}
            </dd>
            <dt>effective read paths ({effectiveRead.length})</dt>
            <dd>
              {effectiveRead.length === 0 ? (
                <span className="muted">none — all reads rejected</span>
              ) : (
                effectiveRead.map((p, i) => (
                  <div key={i}>
                    <code>{p}</code>
                  </div>
                ))
              )}
            </dd>
          </dl>
        </CollapsibleSection>
      </Card>
    </Stack>
  );
}