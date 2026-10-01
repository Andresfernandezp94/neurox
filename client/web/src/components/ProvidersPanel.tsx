// LLM Providers Panel — full CRUD + model discovery (EP-0010).
// Migrated to atomic design in EP-0016 (F4.2): consumes <Card>, <Row>,
// <Stack>, <Badge>, <Button>, <IconButton>, <ErrorBanner> + utility classes.

import { useCallback, useEffect, useState } from "react";
import {
  getProviders,
  setActiveProvider,
  deleteProvider,
  startProvider,
  stopProvider,
  type LlmProviderStatus,
} from "../api/llm";
import { ConfirmDialog } from "../shared/components/ConfirmDialog";
import { putEnvVar, deleteEnvVar } from "../api/env";
import { ProviderLogo } from "../shared/components/ProviderLogo";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Badge } from "../shared/components/atoms/Badge";
import { IconButton } from "../shared/components/atoms/IconButton";
import { Input } from "../shared/components/atoms/Input";

export function ProvidersPanel() {
  const [providers, setProviders] = useState<LlmProviderStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  // EP-0024: tab interna — "providers" (default) o "models".
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const fetchProviders = useCallback(async () => {
    try {
      const data = await getProviders();
      setProviders(data.providers);
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchProviders();
  }, [fetchProviders]);

  const handleSetActive = useCallback(
    async (id: string) => {
      setBusy(`activate:${id}`);
      setError(null);
      try {
        await setActiveProvider(id);
        await fetchProviders();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setBusy(null);
      }
    },
    [fetchProviders],
  );

  // Save an API key to the daemon's env for a provider, then refresh so
  // `configured` updates. Mirrors the sidebar's saveNeuroxKey flow.
  const handleSaveKey = useCallback(
    async (p: LlmProviderStatus, value: string) => {
      const envName = p.api_key_env ?? p.id;
      if (!value) return;
      setBusy(`key:${p.id}`);
      setError(null);
      try {
        await putEnvVar(envName, value);
        await fetchProviders();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setBusy(null);
      }
    },
    [fetchProviders],
  );

  // Clear an API key from the daemon's env for a provider.
  const handleClearKey = useCallback(
    async (p: LlmProviderStatus) => {
      const envName = p.api_key_env ?? p.id;
      setBusy(`key:${p.id}`);
      setError(null);
      try {
        await deleteEnvVar(envName);
        await fetchProviders();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setBusy(null);
      }
    },
    [fetchProviders],
  );

  // EP-0018-04: start/stop a local provider's service.
  const handleToggleService = useCallback(
    async (id: string, action: "start" | "stop") => {
      setBusy(`svc:${action}:${id}`);
      setError(null);
      try {
        const resp =
          action === "start" ? await startProvider(id) : await stopProvider(id);
        // Optimistic: patch the in-memory provider's service_state so the UI
        // reflects the new state without waiting for the refetch.
        setProviders((prev) =>
          prev.map((p) =>
            p.id === id ? { ...p, service_state: resp.service_state } : p,
          ),
        );
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setBusy(null);
        // Refetch in the background to pick up the async probe result.
        void fetchProviders();
      }
    },
    [fetchProviders],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      setBusy(`delete:${id}`);
      setError(null);
      try {
        await deleteProvider(id);
        setDeleteTarget(null);
        await fetchProviders();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setBusy(null);
      }
    },
    [fetchProviders],
  );

  // Local toggles + set-active are wired (handlers defined above) but
  // intentionally hidden in the header per operator request — keep the
  // handlers reachable so we don't have to re-thread them when the UI
  // brings them back. Suppress TS6133 unused-locals lint here.
  void handleSetActive;
  void handleToggleService;

  return (
    <div className="providers-list">
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {loading && <p className="muted">Loading...</p>}

      <p className="muted text-sm providers-panel__description">
        Providers are managed by the neurox daemon and shared with the sidebar. API keys live in the daemon's environment.
      </p>
      {!loading && providers.length === 0 && (
        <p className="muted">No providers reported by the daemon.</p>
      )}

      {providers.map((p) => (
        <ProviderCardSB
          key={p.id}
          provider={p}
          busy={busy === `key:${p.id}`}
          onSaveKey={(value) => handleSaveKey(p, value)}
          onClearKey={() => handleClearKey(p)}
        />
      ))}

      {/* Delete Confirmation */}
      <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete Provider"
        message={`Are you sure you want to delete provider "${deleteTarget}"? This cannot be undone.`}
        confirmLabel="Delete"
        destructive
        onConfirm={() => {
          if (deleteTarget) void handleDelete(deleteTarget);
        }}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}

// ─── ProviderCardSB — sidebar-style provider card ─────────────────────────────
// Mirrors the Quickshell sidebar's AiChatProviderModal card: provider logo +
// name + ACTIVE badge, kind • model, endpoint (mono), key status, and a
// password field with save + clear buttons. Data comes from the daemon.

interface ProviderCardSBProps {
  provider: LlmProviderStatus;
  busy: boolean;
  onSaveKey: (value: string) => void;
  onClearKey: () => void;
}

function ProviderCardSB({ provider: p, busy, onSaveKey, onClearKey }: ProviderCardSBProps) {
  const [keyValue, setKeyValue] = useState("");
  const canSave = keyValue.trim().length > 0 && !busy;

  const save = () => {
    if (!canSave) return;
    onSaveKey(keyValue.trim());
    setKeyValue("");
  };

  return (
    // gap="md" (0.5rem), no "sm": con 4px los tres bloques —header,
    // endpoint y keyrow— se leen pegados. La nota del CSS que decía
    // "6px" describía la intención, no lo que gap-sm realmente aplica.
    <Card gap="md" className={`provider-card ${p.active ? "provider-card--active" : ""}`}>
      {/* Header: logo + name + ACTIVE badge */}
      <Row gap="sm" align="center">
        <ProviderLogo id={p.id} kind={p.kind} className="provider-logo" />
        <strong className="strong" style={{ flex: "1 1 auto" }}>{p.id}</strong>
        {p.active && <Badge className="badge--active">ACTIVE</Badge>}
      </Row>

      {/* endpoint */}
      <div className="provider-endpoint">{p.base_url}</div>

      {/* key entry: password field + save + (clear) */}
      <Row gap="sm" align="stretch" className="provider-keyrow">
        <Input
          type="password"
          placeholder={p.configured ? "••••••••  (key set — enter new to replace)" : "Paste your API key here…"}
          value={keyValue}
          onChange={(e) => setKeyValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && canSave) save();
          }}
          data-testid={`provider-key-input-${p.id}`}
        />
        <IconButton
          icon="IconSave"
          aria-label="Save API key to daemon"
          title="Save API key to daemon"
          disabled={!canSave}
          data-testid={`provider-key-save-${p.id}`}
          onClick={save}
        />
        {p.configured && (
          <IconButton
            icon="IconTrash"
            aria-label="Clear API key from daemon"
            title="Clear API key from daemon"
            variant="danger"
            data-testid={`provider-key-clear-${p.id}`}
            onClick={() => {
              onClearKey();
              setKeyValue("");
            }}
          />
        )}
      </Row>
    </Card>
  );
}