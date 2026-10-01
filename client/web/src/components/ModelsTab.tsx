// ModelsTab — Local GGUF + Hugging Face search + Configure modal. EP-0020-02.
// EP-0024: absorbido como tab interna dentro de ProvidersPanel.
// 2026-09: mismo look and feel que Providers (lista de .provider-card) +
// control Start/Stop del servicio local (ollama serve) via /start /stop.

import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
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
import { IconCheck, IconDownload, IconEdit } from "../shared/components/Icons";
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
  type LlmProviderStatus,
} from "../api/llm";
import { LocalServiceCard } from "./LocalServiceCard";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/** Categoría `unclassified`: el .gguf está en la raíz, sin carpeta que lo
 *  organice. No es una categoría de verdad: es el estado "el operador todavía
 *  no lo organizó", y tiene que leerse distinto en la UI. */
const UNCLASSIFIED = "unclassified";

const CATEGORY_LABELS: Record<string, string> = {
  chat: "Chat",
  embedding: "Embedding",
  rerank: "Rerank",
  vision: "Vision",
  code: "Code",
  audio: "Audio",
  tts: "TTS",
  transcription: "Transcription",
};

/** Label legible de una categoría. Una desconocida se muestra tal cual:
 *  el operador puede crear las carpetas que quiera y la UI tiene que
 *  respetar su nomenclatura en vez deforcerla a un set cerrado. */
function categoryLabel(category: string): string {
  if (category === UNCLASSIFIED) return "Sin categoria";
  return CATEGORY_LABELS[category] ?? category;
}

export function ModelsTab() {
  const [local, setLocal] = useState<LocalModel[]>([]);
  // `null` = el directorio de MODELS_DIR no existe. No es lo mismo que
  // "vacío": cambia por completo el consejo al operador.
  const [modelsDir, setModelsDir] = useState<string | null>(null);
  const [modelsEnvVar, setModelsEnvVar] = useState("NEUROX_MODELS_DIR");
  const [hf, setHf] = useState<HfModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [searching, setSearching] = useState(false);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [editing, setEditing] = useState<LocalModel | null>(null);
  // EP-0025: selector runtime del modelo a cargar.
  const [activeLocalPath, setActiveLocalPath] = useState<string | null>(null);
  const [setting, setSetting] = useState<string | null>(null);

  // Agrupa por categoría. El daemon ya devuelve los resultados ordenados
  // por categoría, así que el orden de aparición respeta el del daemon sin
  // volver a ordenar acá.
  // Solo los GGUF de categoría `chat`: `llama-server` sirve modelos de
  // chat. Ofrecer un embedding (bge-m3, embeddinggemma) seria prometer algo
  // que el servicio no puede hacer.
  const chatModels = useMemo(
    () => local.filter((m) => (m.category || UNCLASSIFIED) === "chat"),
    [local],
  );

  const localByCategory = useMemo(() => {
    const map = new Map<string, LocalModel[]>();
    for (const m of local) {
      const cat = m.category || UNCLASSIFIED;
      const arr = map.get(cat) ?? [];
      arr.push(m);
      map.set(cat, arr);
    }
    return Array.from(map.entries()).map(([category, items]) => ({
      category,
      items,
    }));
  }, [local]);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getLocalModels();
      setLocal(data.models);
      setModelsDir(data.dir);
      setModelsEnvVar(data.env_var);
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
      setActiveLocalPath(localProvider?.local_model_path ?? null);
    } catch {
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
    { id: "nemotron", name: "Nemotron", query: "nemotron", hint: "NVIDIA" },
  ] as const;

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
  const familyOf = useCallback((s: string): string | null => {
    const lower = s.toLowerCase();
    const fam = HF_FAMILIES.find((f) => lower.includes(f.query));
    return fam ? fam.id : null;
  }, []);


  return (
    <div className="providers-list" data-testid="providers-models-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <p className="muted text-sm providers-panel__description">
        GGUF models in MODELS_DIR are served by the local ollama service. Use
        the card above to start/stop it, then pick which model loads on start.
      </p>

      {/* ─── Servicio local ───
          Vive acá y no en la tab Providers: un provider local no tiene API
          key, y mezclarlo con los remotos lo hace indistinguible de algo que
          necesita un key. El lugar donde se configura es donde se ven los
          GGUF que va a servir. */}
      <div className="providers-list__section-head">
        <h4 className="muted">Local service</h4>
      </div>
      <LocalServiceCard chatModels={chatModels} onChanged={load} />

      {/* ─── Local GGUF models ─── */}
      <div className="providers-list__section-head">
        <h4 className="muted">Local GGUF models</h4>
        <span className="muted text-sm">
          {localByCategory.length > 0
            ? `${local.length} in ${localByCategory.length} ${localByCategory.length === 1 ? "category" : "categories"}`
            : `Models: 0`}
        </span>
      </div>

      {modelsDir && (
        <p className="muted text-sm providers-panel__description">
          <code>{modelsDir}</code> · category = subfolder
        </p>
      )}

      {loading && <p className="muted">Loading local models…</p>}

      {!loading && local.length === 0 && (
        <EmptyState>
          {modelsDir === null ? (
            <>
              <EmptyState.Title>Models directory not found</EmptyState.Title>
              <EmptyState.Hint>
                <code>{modelsEnvVar}</code> points at a directory that does not
                exist. Create it, or point it at an existing one from the
                Environment tab. This is NOT the same as an empty directory:
                downloading a model would fail.
              </EmptyState.Hint>
            </>
          ) : (
            <>
              <EmptyState.Title>No local models</EmptyState.Title>
              <EmptyState.Hint>
                <code>{modelsDir}</code> has no .gguf files. Download one from
                Hugging Face below, or organize existing ones into
                subfolders to categorize them.
              </EmptyState.Hint>
            </>
          )}
        </EmptyState>
      )}

      {/* Un grupo por categoría. El orden y el label vienen del daemon, que
          ya los ordena por categoria para que la UI no reshuffle. */}
      {localByCategory.map(({ category, items }) => (
        <Fragment key={category}>
          {items.length > 0 && (
            <div className="providers-list__section-head">
              <h5 className="muted models-tab__category">{categoryLabel(category)}</h5>
              <span className="muted text-sm">{items.length}</span>
            </div>
          )}
          {items.map((m) => {
            const isActive = m.path === activeLocalPath;
            const fam = familyOf(m.filename);
            return (
              <Card
                key={m.path}
                className={`provider-card ${isActive ? "provider-card--active" : ""}`}
              >
                <header className="provider-card__header">
                  <div className="provider-card__identity">
                    {fam ? (
                      <FamilyLogo id={fam} className="provider-logo" />
                    ) : (
                      <ProviderLogo id={m.filename} kind="local" className="provider-logo" />
                    )}
                    <strong className="strong">{m.filename}</strong>
                  </div>
                  <div className="provider-card__actions">
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
                    {isActive && (
                      <Badge className="badge--active">ACTIVE</Badge>
                    )}
                  </div>
                </header>
                <hr className="provider-card__divider" />

                <div className="provider-meta provider-meta--right">{formatSize(m.size_bytes)}</div>
              </Card>
            );
          })}
        </Fragment>
      ))}

      {/* ─── Hugging Face search ─── */}
      <div className="providers-list__section-head">
        <h4 className="muted">Download from Hugging Face</h4>
      </div>
      <Card className="provider-card">
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
              {hf.length} result{hf.length === 1 ? "" : "s"}
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
        {hf.length > 0 && (
          <div
            className="models-panel__hf-list"
            data-testid="models-hf-list"
            style={{
              maxHeight: "26rem",
              overflowY: "auto",
              overflowX: "hidden",
              display: "flex",
              flexDirection: "column",
              gap: 8,
              padding: 4,
            }}
          >
            {hf.map((m) => {
            const fam = familyOf(m.id);
            return (
              <Card key={m.id} className="models-panel__hf-card">
                <Row justify="between" align="center">
                  <Row gap="md" align="center">
                    {fam && <FamilyLogo id={fam} className="provider-logo" />}
                    <strong className="strong">{m.display_name || m.id}</strong>
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
                <Row justify="between" align="center">
                  <div className="muted text-sm">
                    <code>{m.id}</code>
                  </div>
                  <span className="badge badge--down">
                    <IconDownload /> {m.downloads.toLocaleString()}
                  </span>
                </Row>
              </Card>
            );
          })}
          </div>
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