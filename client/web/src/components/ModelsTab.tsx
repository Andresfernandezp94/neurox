// ModelsTab — Local GGUF + Hugging Face search + Configure modal. EP-0020-02.
// EP-0024: absorbido como tab interna dentro de ProvidersPanel.
// Antes era un panel separado (ModelsPanel); ahora se importa como ModelsTab.

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
import { CollapsibleSection } from "../shared/components/molecules/CollapsibleSection";
import { FamilyLogo } from "../shared/components/FamilyLogo";
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
  // EP-0025: selector runtime del modelo a cargar. `activeLocalPath` es
  // el `local_model_path` actual del provider local (típicamente "local-llama").
  // `setting` deshabilita el botón mientras se aplica la update.
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

  // EP-0025: descubrir el modelo activo actual del provider local. Si no
  // hay provider con `local_command` configurado, `activeLocalPath` queda
  // en null y la UI muestra todos los modelos como "no activos".
  const loadActiveModel = useCallback(async () => {
    try {
      const data = await getProviders();
      const localProvider = data.providers.find((p) => p.local_command);
      setActiveLocalPath(localProvider?.local_model_path ?? null);
    } catch {
      setActiveLocalPath(null);
    }
  }, []);

  // EP-0025: aplicar el path seleccionado al provider local. NO recarga
  // el modelo en runtime (eso requiere Stop+Start). El usuario debe
  // ir a la tab "providers" y arrancar el servicio.
  const handleSetActive = useCallback(
    async (filename: string, path: string) => {
      setSetting(filename);
      setError(null);
      try {
        // Hardcoded al provider "local-llama" (hoy es el único con
        // local_command configurado). En el futuro se puede generalizar
        // a "elegir provider destino".
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

  useEffect(() => {
    void load();
  }, [load]);

  // EP-0025: cargar el modelo activo al mount.
  useEffect(() => {
    void loadActiveModel();
  }, [loadActiveModel]);

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

  // EP-0025: debounced search-as-you-type. After 400ms of inactivity on
  // the `search` input, fire doSearch automatically. The cleanup cancels
  // the pending timer if the user keeps typing, so we only run the
  // last one. Empty query clears the list immediately.
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

  // EP-0025: prefetch al mount. Carga "gguf" como query default para
  // que la lista esté populada y los filtros funcionen sin tener que
  // tipear primero. "gguf" devuelve los 30 modelos GGUF más populares,
  // lo que da cobertura amplia para los family filters.
  useEffect(() => {
    setSearch("gguf");
    void doSearch("gguf");
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // EP-0025: familias hardcoded de Hugging Face. Las cards funcionan como
  // filtros OR sobre la lista de resultados del search. Click toggle:
  //   - si el filtro está activo → lo quita
  //   - si NO está activo → lo agrega
  // La lista filtrada se computa con `filteredHf`. Si no hay filtros
  // activos, la lista muestra todos los resultados del search.
  // Cada familia mapea a un query que el endpoint `/v1/llm/models/hf`
  // acepta (devuelve los 30 modelos más relevantes).
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

  // EP-0025: intersección OR. Un modelo aparece si su id contiene
  // CUALQUIERA de los filtros activos (case-insensitive). Si no hay
  // filtros activos, la lista es la búsqueda completa.
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

  return (
    <Stack gap="md" data-testid="providers-models-tab">
      {/* EP-0025: header consistente con SandboxTab (mismo panel__header /
          panel__title / panel__subtitle) para look and feel unificado. */}
      <header className="sandbox-panel__header">
        <div>
          <h3 className="sandbox-panel__title">Models</h3>
          <p className="sandbox-panel__subtitle">
            Manage local GGUF models and download new ones from Hugging Face.
            Select a model below, then press Start in the provider card to load it.
          </p>
        </div>
      </header>

      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card className="sandbox-panel__card">
        {/* ─── Local models ─── */}
        <CollapsibleSection
          title="Local"
          badge={
            <span
              className={
                local.length > 0
                  ? "collapsible__badge collapsible__badge--accent"
                  : "collapsible__badge"
              }
            >
              {local.length}
            </span>
          }
          hint={dir ? `dir: ${dir}` : "GGUF files in MODELS_DIR"}
          defaultOpen={true}
          data-testid="models-local-section"
        >
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
            return (
              <Card key={m.filename} className="models-panel__local-card">
                <Row justify="between" align="center">
                  <Row gap="md">
                    <strong className="models-panel__filename">{m.filename}</strong>
                    <span className="muted">{formatSize(m.size_bytes)}</span>
                    {/* EP-0025: badge del modelo activo. */}
                    {isActive && (
                      <span className="badge badge--ok" data-testid="model-active-badge">
                        Active
                      </span>
                    )}
                  </Row>
                  <Row gap="sm">
                    {/* EP-0025: botón para setear este modelo como el que se
                        cargará al hacer Start del provider local. */}
                    {!isActive && (
                      <Button
                        variant="secondary"
                        size="sm"
                        disabled={setting === m.filename}
                        onClick={() => handleSetActive(m.filename, m.path)}
                        data-testid="set-active-button"
                      >
                        {setting === m.filename ? "Setting…" : "Set as active"}
                      </Button>
                    )}
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => setEditing(m)}
                    >
                      Configure
                    </Button>
                  </Row>
                </Row>
              </Card>
            );
          })}
        </CollapsibleSection>

        {/* ─── Hugging Face search ─── */}
        <CollapsibleSection
          title="Hugging Face search"
          badge={
            <span className="collapsible__badge">
              {hf.length > 0 ? hf.length : "—"}
            </span>
          }
          hint="Search GGUF models and download them to MODELS_DIR"
          defaultOpen={false}
          data-testid="models-hf-section"
        >
          {/* EP-0025: cards de families como filtros OR clickeables. Click
              toggle: aparecen/desaparecen de `activeFilters`. La card
              activa se distingue con border accent. */}
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
          {/* EP-0025: search-as-you-type (debounced 400ms). No Search button
              needed — typing alone updates the list. The pill on the right
              shows live status (idle / searching / N results, M filtered). */}
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
          {/* EP-0025: scrollable list capped to ~3 visible cards. The wrapper
              shows vertical scroll only when results exceed the visible area;
              horizontal scroll is disabled so long model ids don't push the
              layout sideways. Uses `filteredHf` (post-filter) so the family
              cards above act as OR filters over the search results. */}
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
                      <strong className="models-panel__display-name">
                        {m.display_name || m.id}
                      </strong>
                      <span className="muted">{m.downloads.toLocaleString()} ↓</span>
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
                  <div className="muted models-panel__hf-id">
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
        </CollapsibleSection>
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
    </Stack>
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
