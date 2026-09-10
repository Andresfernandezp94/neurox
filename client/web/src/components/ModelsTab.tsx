// ModelsTab — Local GGUF + Hugging Face search + Configure modal. EP-0020-02.
// EP-0024: absorbido como tab interna dentro de ProvidersPanel.
// 2026-09: mismo look and feel que Providers (lista de .provider-card) +
// control Start/Stop del servicio local (ollama serve) via /start /stop.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { Input } from "../shared/components/atoms/Input";
import { Label } from "../shared/components/atoms/Label";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { SearchBar } from "../shared/components/molecules/SearchBar";
import { Badge } from "../shared/components/atoms/Badge";
import { ProviderLogo } from "../shared/components/ProviderLogo";
import { FamilyLogo } from "../shared/components/FamilyLogo";
import { IconCheck, IconEdit, IconPause, IconPlay } from "../shared/components/Icons";
import {
  getLocalModels,
  searchHfModels,
  downloadModel,
  getModelConfig,
  putModelConfig,
  type LocalModel,
  type HfModel,
  type ModelConfig,
} from "../api/models";
import {
  getProviders,
  updateProvider,
  startProvider,
  stopProvider,
  type LlmProviderStatus,
} from "../api/llm";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function ModelsTab() {
  const [local, setLocal] = useState<LocalModel[]>([]);
  const [dir, setDir] = useState<string>("");
  const [hf, setHf] = useState<HfModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [searching, setSearching] = useState(false);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [editing, setEditing] = useState<LocalModel | null>(null);
  // Servicio local (ollama serve): provider con local_command. Start/Stop.
  const [service, setService] = useState<LlmProviderStatus | null>(null);
  const [svcBusy, setSvcBusy] = useState(false);
  // EP-0025: selector runtime del modelo a cargar.
  const [activeLocalPath, setActiveLocalPath] = useState<string | null>(null);
  const [setting, setSetting] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getLocalModels();
      setLocal(data.models);
      setDir(data.dir);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  // EP-0025: descubrir el modelo activo actual del provider local y el
  // servicio local. Orden de preferencia: 1) provider con local_command
  // (orquestado por el daemon), 2) provider con service_state, 3) provider
  // en loopback o de tipo ollama/local (aún sin local_command → la card
  // igual se muestra, con la pista de que falta orquestar).
  const isLocalProvider = useCallback((p: LlmProviderStatus) => {
    if (p.local_command) return true;
    if (p.service_state != null) return true;
    const host = p.base_url.toLowerCase();
    if (
      host.includes("127.0.0.1") ||
      host.includes("localhost") ||
      host.includes("0.0.0.0")
    ) {
      return true;
    }
    const id = p.id.toLowerCase();
    const kind = p.kind.toLowerCase();
    return id.startsWith("local") || kind === "ollama" || kind.includes("local");
  }, []);

  const loadProviders = useCallback(async () => {
    try {
      const data = await getProviders();
      const sorted = [...data.providers]
        .filter(isLocalProvider)
        .sort((a, b) => {
          const rank = (p: LlmProviderStatus) =>
            p.local_command ? 0 : p.service_state != null ? 1 : 2;
          return rank(a) - rank(b);
        });
      const localProvider = sorted[0] ?? null;
      setService(localProvider);
      setActiveLocalPath(localProvider?.local_model_path ?? null);
    } catch {
      setService(null);
      setActiveLocalPath(null);
    }
  }, [isLocalProvider]);

  // EP-0025: aplicar el path seleccionado al provider local. NO recarga
  // el modelo en runtime (eso requiere Stop+Start).
  const handleSetActive = useCallback(
    async (filename: string, path: string) => {
      setSetting(filename);
      setError(null);
      try {
        await updateProvider("local-llama", { local_model_path: path });
        setActiveLocalPath(path);
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setSetting(null);
      }
    },
    [],
  );

  // EP-0018-04: encender/apagar el servicio local (ollama serve) desde
  // el frontend. Patch optimista + refetch en background.
  const handleToggleService = useCallback(async () => {
    if (!service) return;
    const action =
      service.service_state === "running" || service.service_state === "ready"
        ? "stop"
        : "start";
    setSvcBusy(true);
    setError(null);
    try {
      const resp =
        action === "start" ? await startProvider(service.id) : await stopProvider(service.id);
      setService((prev) =>
        prev ? { ...prev, service_state: resp.service_state } : prev,
      );
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSvcBusy(false);
      void loadProviders();
    }
  }, [service, loadProviders]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  const doSearch = useCallback(async (override?: string) => {
    const query = (override ?? search).trim();
    if (!query) return;
    setSearching(true);
    setError(null);
    try {
      const data = await searchHfModels(query);
      setHf(data.models);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSearching(false);
    }
  }, [search]);

  // EP-0025: debounced search-as-you-type.
  useEffect(() => {
    if (!search.trim()) {
      setHf([]);
      setSearching(false);
      return;
    }
    const handle = setTimeout(() => {
      void doSearch();
    }, 400);
    return () => clearTimeout(handle);
  }, [search, doSearch]);

  // EP-0025: prefetch al mount — "gguf" devuelve los populares.
  useEffect(() => {
    setSearch("gguf");
    void doSearch("gguf");
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const HF_FAMILIES = [
    { id: "qwen", name: "Qwen", query: "qwen", hint: "Alibaba" },
    { id: "llama", name: "Llama", query: "llama", hint: "Meta" },
    { id: "mistral", name: "Mistral", query: "mistral", hint: "Mistral AI" },
    { id: "gemma", name: "Gemma", query: "gemma", hint: "Google" },
    { id: "phi", name: "Phi", query: "phi", hint: "Microsoft" },
    { id: "deepseek", name: "DeepSeek", query: "deepseek", hint: "DeepSeek" },
    { id: "whisper", name: "Whisper", query: "whisper", hint: "OpenAI (audio)" },
  ] as const;

  const [activeFilters, setActiveFilters] = useState<Set<string>>(
    () => new Set(),
  );

  const toggleFilter = useCallback((id: string) => {
    setActiveFilters((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const filteredHf = useMemo(() => {
    if (activeFilters.size === 0) return hf;
    const filters = Array.from(activeFilters);
    return hf.filter((m) => {
      const id = m.id.toLowerCase();
      return filters.some((f) => id.includes(f.toLowerCase()));
    });
  }, [hf, activeFilters]);

  const handleDownload = useCallback(async (id: string) => {
    setDownloading(id);
    setError(null);
    try {
      await downloadModel(id);
      await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setDownloading(null);
    }
  }, [load]);

  // Logo de familia para el filename local (ej. qwen2.5-...gguf → Qwen).
  const familyOf = useCallback((filename: string): string | null => {
    const lower = filename.toLowerCase();
    const fam = HF_FAMILIES.find((f) => lower.startsWith(f.id));
    return fam ? fam.id : null;
  }, []);

  const svcState = service?.service_state ?? "stopped";
  const svcRunning = svcState === "running" || svcState === "ready";
  const svcStarting = svcState === "starting";

  return (
    <div className="providers-list" data-testid="providers-models-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {/* ─── Servicio local (ollama serve) ─── */}
      {service ? (
        <Card
          className={`provider-card ${service.active || svcRunning ? "provider-card--active" : ""}`}
        >
          <header className="provider-card__header">
            <div className="provider-card__identity">
              <ProviderLogo
                id={service.id}
                kind={service.kind}
                className="provider-logo"
              />
              <span className="provider-card__title">{service.id}</span>
            </div>
            <div className="provider-card__actions">
              <span
                className={`provider-card__dot provider-card__dot--${svcRunning || svcStarting ? "active" : "inactive"}`}
                title={svcState}
              />
              <span className="provider-card__divider-v" />
              <button
                type="button"
                disabled={svcBusy || svcStarting}
                className={`provider-action ${svcRunning ? "provider-action--delete" : "provider-action--save"}`}
                title={svcRunning ? "Stop local service" : "Start local service"}
                data-testid="local-svc-toggle"
                onClick={handleToggleService}
              >
                {svcRunning ? <IconPause /> : <IconPlay />}
              </button>
            </div>
          </header>
          <hr className="provider-card__divider" />

          <Row gap="sm" align="center">
            <div className="provider-meta">
              {service.kind} &nbsp;•&nbsp; {service.model}
            </div>
            <span className="muted text-sm" style={{ marginLeft: "auto" }}>
              {svcState}
            </span>
          </Row>
          <div className="provider-endpoint">{service.base_url}</div>
          {service.local_command ? (
            <div className="muted text-sm">
              run: <code>{service.local_command}</code>
            </div>
          ) : (
            <div className="muted text-sm">
              not orchestrated by the daemon yet: set a{" "}
              <code>local_command</code> on this provider (Providers tab) so
              Start/Stop can control the service.
            </div>
          )}
        </Card>
      ) : (
        <p className="muted text-sm">
          No local service configured: no provider has a local_command to
          start/stop. Configure one under the Providers tab.
        </p>
      )}

      <h3 className="provider-card__title">Models</h3>
      <p className="muted text-sm">
        GGUF models in MODELS_DIR are served by the local ollama service. Use
        the card above to start/stop it, then pick which model loads on start.
      </p>

      {/* ─── Local GGUF models ─── */}
      <h4 className="muted">
        Local GGUF models ({local.length})
        {dir ? (
          <span className="text-sm">{` — dir: ${dir}`}</span>
        ) : null}
      </h4>
      {loading && <p className="muted">Loading local models…</p>}
      {!loading && local.length === 0 && (
        <EmptyState>
          <EmptyState.Title>No local models</EmptyState.Title>
          <EmptyState.Hint>
            The MODELS_DIR is empty. Download a model from Hugging Face.
          </EmptyState.Hint>
        </EmptyState>
      )}
      {local.map((m) => {
        const isActive = m.path === activeLocalPath;
        const fam = familyOf(m.filename);
        return (
          <Card
            key={m.filename}
            className={`provider-card ${isActive ? "provider-card--active" : ""}`}
          >
            <header className="provider-card__header">
              <div className="provider-card__identity">
                {fam ? (
                  <FamilyLogo id={fam} className="provider-logo" />
                ) : (
                  <ProviderLogo id={m.filename} kind="local" className="provider-logo" />
                )}
                <span className="provider-card__title">{m.filename}</span>
              </div>
              <div className="provider-card__actions">
                <span
                  className={`provider-card__dot provider-card__dot--${isActive ? "active" : "inactive"}`}
                  title={isActive ? "active model" : "inactive"}
                  data-testid="model-active-dot"
                />
                <span className="provider-card__divider-v" />
                {!isActive && (
                  <button
                    type="button"
                    disabled={setting === m.filename}
                    className="provider-action provider-action--save"
                    title="Set as active model"
                    data-testid="set-active-button"
                    onClick={() => handleSetActive(m.filename, m.path)}
                  >
                    {setting === m.filename ? <span>…</span> : <IconCheck />}
                  </button>
                )}
                <button
                  type="button"
                  className="provider-action"
                  title="Configure model"
                  onClick={() => setEditing(m)}
                >
                  <IconEdit />
                </button>
              </div>
            </header>
            <hr className="provider-card__divider" />

            <Row gap="sm" align="center">
              <div className="provider-meta">{formatSize(m.size_bytes)}</div>
              {isActive && (
                <Badge className="badge--active" data-testid="model-active-badge">
                  ACTIVE
                </Badge>
              )}
            </Row>
            <div className="provider-endpoint">{m.path}</div>
          </Card>
        );
      })}

      {/* ─── Hugging Face search ─── */}
      <h4 className="muted">Download from Hugging Face</h4>
      <Card className="provider-card">
        <div
          className="models-panel__family-grid"
          data-testid="models-family-grid"
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(140px, 1fr))",
            gap: 8,
          }}
        >
          {HF_FAMILIES.map((f) => {
            const isActive = activeFilters.has(f.id);
            return (
              <Card
                key={f.id}
                className="models-panel__family-card"
                data-testid={`models-family-${f.id}`}
                data-active={isActive ? "true" : undefined}
                onClick={() => toggleFilter(f.id)}
                style={{
                  background: "var(--bg)",
                  border: isActive
                    ? "2px solid var(--accent, #6366f1)"
                    : "1px solid var(--border-color, transparent)",
                  cursor: "pointer",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <FamilyLogo id={f.id} className="models-panel__family-logo" />
                  <div>
                    <div className="models-panel__family-name">{f.name}</div>
                    <div className="muted models-panel__family-hint">{f.hint}</div>
                  </div>
                </div>
              </Card>
            );
          })}
        </div>

        <Row gap="sm" align="center">
          <div style={{ flex: 1 }}>
            <SearchBar
              placeholder="Search Hugging Face (e.g. qwen2.5)"
              value={search}
              onChange={setSearch}
            />
          </div>
          {searching && (
            <span className="badge badge--muted" data-testid="hf-searching-badge">
              searching…
            </span>
          )}
          {!searching && search.trim() && hf.length > 0 && (
            <span className="badge badge--accent" data-testid="hf-results-badge">
              {activeFilters.size > 0
                ? `${filteredHf.length}/${hf.length} filtered`
                : `${hf.length} result${hf.length === 1 ? "" : "s"}`}
            </span>
          )}
        </Row>

        {searching && <p className="muted">Searching…</p>}
        {!searching && hf.length === 0 && search && (
          <EmptyState>
            <EmptyState.Title>No results</EmptyState.Title>
            <EmptyState.Hint>Try a different query.</EmptyState.Hint>
          </EmptyState>
        )}
        {filteredHf.length > 0 && (
          <div
            className="models-panel__hf-list"
            data-testid="models-hf-list"
            style={{
              maxHeight: 360,
              overflowY: "auto",
              overflowX: "hidden",
              display: "flex",
              flexDirection: "column",
              gap: 8,
              padding: 4,
              border: "1px solid var(--border-color, #3a3a3a)",
              borderRadius: 8,
            }}
          >
            {filteredHf.map((m) => (
              <Card key={m.id} className="models-panel__hf-card">
                <Row justify="between" align="center">
                  <Row gap="md">
                    <strong className="strong">{m.display_name || m.id}</strong>
                    <span className="muted text-sm">{m.downloads.toLocaleString()} ↓</span>
                    {m.gated && (
                      <span className="badge badge--warn">gated</span>
                    )}
                  </Row>
                  <Button
                    variant="primary"
                    size="sm"
                    disabled={downloading === m.id}
                    onClick={() => handleDownload(m.id)}
                  >
                    {downloading === m.id ? "…" : "Download"}
                  </Button>
                </Row>
                <div className="muted text-sm">
                  <code>{m.id}</code>
                </div>
              </Card>
            ))}
          </div>
        )}
        {!searching && hf.length > 0 && filteredHf.length === 0 && (
          <EmptyState>
            <EmptyState.Title>No matches with current filters</EmptyState.Title>
            <EmptyState.Hint>
              Click an active family card above to remove the filter.
            </EmptyState.Hint>
          </EmptyState>
        )}
      </Card>

      {editing && (
        <ConfigureModal
          model={editing}
          onClose={() => setEditing(null)}
          onSaved={async () => {
            setEditing(null);
            await load();
          }}
        />
      )}
    </div>
  );
}

interface ConfigureModalProps {
  model: LocalModel;
  onClose: () => void;
  onSaved: () => void;
}

function ConfigureModal({ model, onClose, onSaved }: ConfigureModalProps) {
  const [config, setConfig] = useState<ModelConfig>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const data = await getModelConfig(model.filename);
        setConfig(data);
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setLoading(false);
      }
    })();
  }, [model.filename]);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await putModelConfig(model.filename, config);
      onSaved();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <Card className="modal models-panel__configure-modal" onClick={(e: React.MouseEvent) => e.stopPropagation()}>
        <h3>Configure {model.filename}</h3>
        {error && <ErrorBanner>{error}</ErrorBanner>}
        {loading && <p className="muted">Loading config…</p>}
        {!loading && (
          <Stack gap="sm">
            <Row justify="between" align="center">
              <Label>temperature</Label>
              <Input
                type="number"
                step={0.05}
                min={0}
                max={2}
                value={config.temperature ?? ""}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    temperature: parseFloat(e.target.value) || undefined,
                  })
                }
              />
            </Row>
            <Row justify="between" align="center">
              <Label>top_p</Label>
              <Input
                type="number"
                step={0.05}
                min={0}
                max={1}
                value={config.top_p ?? ""}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    top_p: parseFloat(e.target.value) || undefined,
                  })
                }
              />
            </Row>
            <Row justify="between" align="center">
              <Label>top_k</Label>
              <Input
                type="number"
                min={0}
                value={config.top_k ?? ""}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    top_k: parseInt(e.target.value, 10) || undefined,
                  })
                }
              />
            </Row>
            <Row justify="between" align="center">
              <Label>max_tokens</Label>
              <Input
                type="number"
                min={1}
                value={config.max_tokens ?? ""}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    max_tokens: parseInt(e.target.value, 10) || undefined,
                  })
                }
              />
            </Row>
            <Row justify="between" align="center">
              <Label>tokens_per_second</Label>
              <Input
                type="number"
                min={0}
                value={config.tokens_per_second ?? ""}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    tokens_per_second:
                      parseInt(e.target.value, 10) || undefined,
                  })
                }
              />
            </Row>
            <Stack gap="sm">
              <Label>stop_sequences (one per line)</Label>
              <textarea
                className="input"
                rows={3}
                value={(config.stop_sequences ?? []).join("\n")}
                onChange={(e) =>
                  setConfig({
                    ...config,
                    stop_sequences: e.target.value
                      .split("\n")
                      .map((s) => s.trim())
                      .filter((s) => s.length > 0),
                  })
                }
              />
            </Stack>
            <Stack gap="sm">
              <Label>system</Label>
              <textarea
                className="input"
                rows={3}
                value={config.system ?? ""}
                onChange={(e) =>
                  setConfig({ ...config, system: e.target.value })
                }
              />
            </Stack>
          </Stack>
        )}
        <Row justify="end" gap="sm">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={loading || saving}
            onClick={handleSave}
          >
            {saving ? "Saving…" : "Save"}
          </Button>
        </Row>
      </Card>
    </div>
  );
}