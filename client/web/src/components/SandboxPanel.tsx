// SandboxPanel — runtime-patch the daemon's sandbox config via PUT /v1/sandbox.
// EP-0020-02. Follows the atomic design pattern (Card, Row, Stack, Button, Input).

import { useCallback, useEffect, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { Input } from "../shared/components/atoms/Input";
import { Label } from "../shared/components/atoms/Label";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { putSandbox, type SandboxConfig } from "../api/sandbox";

export function SandboxPanel() {
  const [config, setConfig] = useState<SandboxConfig | null>(null);
  const [draft, setDraft] = useState<SandboxConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await putSandbox({}); // empty patch returns current config
      setConfig(data);
      setDraft(data);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

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

  if (loading) return <p className="muted">Loading sandbox config…</p>;
  if (!draft) {
    return (
      <EmptyState>
        <EmptyState.Title>Sandbox config unavailable</EmptyState.Title>
        <EmptyState.Hint>{error ?? "No data."}</EmptyState.Hint>
      </EmptyState>
    );
  }

  const dirty =
    config &&
    (config.enabled !== draft.enabled ||
      JSON.stringify(config.writable_paths) !==
        JSON.stringify(draft.writable_paths) ||
      config.max_recursion_depth !== draft.max_recursion_depth);

  return (
    <Stack gap="md">
      <h3 className="models-section__title">Sandbox (runtime patch)</h3>
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {saved && !dirty && (
        <p className="muted">Saved. daemon reloaded template context.</p>
      )}

      <Card className="sandbox-panel__card">
        <Row justify="between" align="center">
          <Label htmlFor="sandbox-enabled">
            <strong>enabled</strong>
          </Label>
          <input
            id="sandbox-enabled"
            type="checkbox"
            checked={draft.enabled}
            onChange={(e) =>
              setDraft({ ...draft, enabled: e.target.checked })
            }
          />
        </Row>

        <Stack gap="sm">
          <Label htmlFor="sandbox-paths">
            <strong>writable_paths</strong>
            <span className="muted"> (one per line; use $&#123;workspace&#125;)</span>
          </Label>
          <textarea
            id="sandbox-paths"
            className="input sandbox-panel__paths"
            rows={5}
            value={draft.writable_paths.join("\n")}
            onChange={(e) =>
              setDraft({
                ...draft,
                writable_paths: e.target.value
                  .split("\n")
                  .map((s) => s.trim())
                  .filter((s) => s.length > 0),
              })
            }
          />
        </Stack>

        <Row justify="between" align="center">
          <Label htmlFor="sandbox-depth">
            <strong>max_recursion_depth</strong>
          </Label>
          <Input
            id="sandbox-depth"
            type="number"
            min={1}
            value={draft.max_recursion_depth}
            onChange={(e) =>
              setDraft({
                ...draft,
                max_recursion_depth: Math.max(1, parseInt(e.target.value, 10) || 1),
              })
            }
          />
        </Row>

        <Row justify="end" gap="sm">
          <Button
            variant="primary"
            disabled={!dirty || saving}
            onClick={handleSave}
          >
            {saving ? "Saving…" : "Apply"}
          </Button>
        </Row>
      </Card>
    </Stack>
  );
}
