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
import { IconCheck, IconDownload } from "../shared/components/Icons";
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
import { LocalServiceCard, LOCAL_SERVICE_ID } from "./LocalServiceCard";

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

/**
 * Familias de modelo que el selector de logo reconoce por substring del
 * nombre del archivo.
 *
 * A scope de modulo: `familyOf` la usan el componente y `ConfigurePanel`,
 * que vive fuera del componente.
 */
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

/**
 * Logo de familia para un filename (ej. `qwen2.5-...gguf` → Qwen).
 *
 * A scope de modulo y no dentro del componente: `ConfigurePanel` vive fuera
 * y lo necesita, y duplicar la tabla para que cada uno tenga la suya es
 * exactamente el tipo de copia que después se desincroniza.
 */
function familyOf(s: string): string | null {
  const lower = s.toLowerCase();
  const fam = HF_FAMILIES.find((f) => lower.includes(f.query));
  return fam ? fam.id : null;
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
      <div className="models-tab__cols">
        <section className="models-tab__col">
          <div className="providers-list__section-head">
            <h4 className="muted">Local service</h4>
          </div>
          <LocalServiceCard chatModels={chatModels} onChanged={load} />
        </section>

        <section className="models-tab__col">
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
          <div className="models-panel__hf-scroll" data-testid="models-hf-scroll">
          {/* Lista interior: flow normal. El scroll lo tiene el wrapper de
              arriba, con alto fijo. */}
          <div
            className="models-panel__hf-list"
            data-testid="models-hf-list"
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
          </div>
        )}
      </Card>

        </section>
      </div>

      {/* ─── Modelos locales + configuración ───
          Dos columnas: la lista a la izquierda, el editor a la derecha.
          El editor NO es un modal: antes había que cerrar un overlay para
          ver los parámetros, y comparar dos modelos requería abrir, leer,
          cerrar y abrir el otro. Con el editor siempre visible, seleccionar
          un modelo muestra su config al lado y el operador puede ir de uno
          a otro sin perder el contexto de la lista. */}
      <div className="models-tab__cols models-tab__cols--models">
        <section className="models-tab__col">
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
                    <code>{modelsEnvVar}</code> points at a directory that does
                    not exist. Create it, or point it at an existing one from
                    the Environment tab. This is NOT the same as an empty
                    directory: downloading a model would fail.
                  </EmptyState.Hint>
                </>
              ) : (
                <>
                  <EmptyState.Title>No local models</EmptyState.Title>
                  <EmptyState.Hint>
                    <code>{modelsDir}</code> has no .gguf files. Download one
                    from Hugging Face above, or organize existing ones into
                    subfolders to categorize them.
                  </EmptyState.Hint>
                </>
              )}
            </EmptyState>
          )}

          {/* Lista densa: una fila por modelo, seleccionable. El click en
              la fila la selecciona y actualiza el editor de la derecha; el
              botón de lapic es redundante con eso y queda solo para el
              "set as active", que es otra acción. */}
          {localByCategory.map(({ category, items }) => (
            <Fragment key={category}>
              {items.length > 0 && (
                <div className="models-tab__cat-head">
                  <span className="models-tab__category">{categoryLabel(category)}</span>
                  <span className="muted text-sm">{items.length}</span>
                </div>
              )}
              <div className="models-tab__model-list">
              {items.map((m) => {
                const isActive = m.path === activeLocalPath;
                const isSelected = editing?.path === m.path;
                const fam = familyOf(m.filename);
                return (
                  <button
                    key={m.path}
                    type="button"
                    className={`models-tab__model-row${isSelected ? " models-tab__model-row--selected" : ""}${isActive ? " models-tab__model-row--active" : ""}`}
                    aria-pressed={isSelected}
                    title={m.path}
                    data-testid={`model-row-${m.filename}`}
                    onClick={() => setEditing(m)}
                    // Un <button> nativo dispara click con Enter y Espacio
                    // en el navegador, pero no con `fireEvent.keyDown` en
                    // jsdom. El onKeyDown explícito cubre ambos caminos y
                    // deja el comportamiento igual en los dos entornos.
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        setEditing(m);
                      }
                    }}
                  >
                    {fam ? (
                      <FamilyLogo id={fam} className="provider-logo" />
                    ) : (
                      <ProviderLogo id={m.filename} kind="local" className="provider-logo" />
                    )}
                    <span className="models-tab__model-name">{m.filename}</span>
                    <span className="muted text-sm models-tab__model-size">
                      {formatSize(m.size_bytes)}
                    </span>
                    {isActive && <Badge className="badge--active">ACTIVE</Badge>}
                  </button>
                );
              })}
              </div>
            </Fragment>
          ))}
        </section>

        {/* Editor, siempre visible. Sin selección muestra un hint en vez de
            desaparecer: si la caja no está, el operador no sabe que puede
            configurar nada. */}
        <section className="models-tab__col">
          <ConfigurePanel
            model={editing}
            isActive={editing ? editing.path === activeLocalPath : false}
            onSetActive={(path) => setActiveLocalPath(path)}
            onSaved={async () => {
              await load();
            }}
          />
        </section>
      </div>

    </div>
  );
}
interface ConfigurePanelProps {
  /** Modelo seleccionado. `null` muestra un hint: el panel siempre está,
   *  aunque todavía no haya nada que configurar. */
  model: LocalModel | null;
  /** Si este modelo es el que carga al arrancar el servicio. */
  isActive: boolean;
  /** EP-0025: aplicar el path al provider local. NO recarga el modelo en
   *  runtime (eso requiere Stop+Start). */
  onSetActive: (path: string) => Promise<void> | void;
  onSaved: () => Promise<void> | void;
}

/**
 * Editor de parámetros de sampling de un modelo local.
 *
 * Inline y siempre visible, no un modal. Antes había que cerrar un overlay
 * para ver los parámetros, y comparar dos modelos exigía abrir, leer,
 * cerrar y abrir el otro. Con el panel al lado, seleccionar un modelo
 * muestra su config y el operador puede pasar de uno a otro sin perder el
 * contexto de la lista.
 */
function ConfigurePanel({
  model,
  isActive,
  onSetActive,
  onSaved,
}: ConfigurePanelProps) {
  const [config, setConfig] = useState<ModelConfig>({});
  const [setting, setSetting] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Cambiar de modelo pide la config de ese modelo. El flag `cancelled`
  // descarta la respuesta si el operador ya clickeó otro: el fetch por
  // filename no se cancela, pero el resultado tardío no debe pisar el
  // modelo nuevo.
  useEffect(() => {
    if (!model) {
      setConfig({});
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    void (async () => {
      try {
        const data = await getModelConfig(model.filename);
        if (!cancelled) setConfig(data);
      } catch (e) {
        if (!cancelled) setError((e as Error).message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [model]);

  const handleSave = async () => {
    if (!model) return;
    setSaving(true);
    setError(null);
    try {
      await putModelConfig(model.filename, config);
      await onSaved();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const handleSetActive = async () => {
    if (!model) return;
    setSetting(true);
    setError(null);
    try {
      // El id del servicio local lo define LocalServiceCard. Antes vivía
      // hardcodeado acá como "local-llama" y ya no coincidía.
      await updateProvider(LOCAL_SERVICE_ID, { local_model_path: model.path });
      await onSetActive(model.path);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSetting(false);
    }
  };

  const asNum = (v: string) => {
    const n = parseFloat(v);
    return Number.isFinite(n) ? n : undefined;
  };
  const asInt = (v: string) => {
    const n = parseInt(v, 10);
    return Number.isFinite(n) ? n : undefined;
  };

  return (
    <Card className="provider-card models-tab__config" data-testid="configure-panel">
      <div className="providers-list__section-head">
        <h4 className="muted">Configuration</h4>
        {model && (
          <span className="muted text-sm models-tab__config-cat">
            {model.category && model.category !== "unclassified"
              ? model.category
              : "sin categoría"}
          </span>
        )}
      </div>

      {!model ? (
        <EmptyState>
          <EmptyState.Title>No model selected</EmptyState.Title>
          <EmptyState.Hint>
            Pick a model on the left to see and edit its sampling parameters.
          </EmptyState.Hint>
        </EmptyState>
      ) : (
        <>
          <div className="models-tab__config-head">
            {familyOf(model.filename) ? (
              <FamilyLogo
                id={familyOf(model.filename) as string}
                className="provider-logo"
              />
            ) : (
              <ProviderLogo
                id={model.filename}
                kind="local"
                className="provider-logo"
              />
            )}
            <div className="models-tab__config-name">
              <strong className="strong">{model.filename}</strong>
              <span className="muted text-sm">{formatSize(model.size_bytes)}</span>
            </div>
            {isActive ? (
              <Badge className="badge--active">ACTIVE</Badge>
            ) : (
              <button
                type="button"
                disabled={setting}
                className="provider-action provider-action--save"
                title="Set as active model"
                aria-label={`Set ${model.filename} as active model`}
                data-testid="set-active-button"
                onClick={() => void handleSetActive()}
              >
                {setting ? <span>…</span> : <IconCheck />}
              </button>
            )}
          </div>

          {error && <ErrorBanner>{error}</ErrorBanner>}
          {loading && <p className="muted text-sm">Loading config…</p>}

          {!loading && (
            <Stack gap="sm">
              <div className="models-tab__field">
                <Label htmlFor="cfg-temperature">temperature</Label>
                <Input
                  id="cfg-temperature"
                  type="number"
                  step={0.05}
                  min={0}
                  max={2}
                  value={config.temperature ?? ""}
                  onChange={(e) =>
                    setConfig({ ...config, temperature: asNum(e.target.value) })
                  }
                  data-testid="cfg-temperature"
                />
              </div>
              <div className="models-tab__field">
                <Label htmlFor="cfg-top-p">top_p</Label>
                <Input
                  id="cfg-top-p"
                  type="number"
                  step={0.05}
                  min={0}
                  max={1}
                  value={config.top_p ?? ""}
                  onChange={(e) =>
                    setConfig({ ...config, top_p: asNum(e.target.value) })
                  }
                  data-testid="cfg-top-p"
                />
              </div>
              <div className="models-tab__field">
                <Label htmlFor="cfg-top-k">top_k</Label>
                <Input
                  id="cfg-top-k"
                  type="number"
                  min={0}
                  value={config.top_k ?? ""}
                  onChange={(e) =>
                    setConfig({ ...config, top_k: asInt(e.target.value) })
                  }
                  data-testid="cfg-top-k"
                />
              </div>
              <div className="models-tab__field">
                <Label htmlFor="cfg-max-tokens">max_tokens</Label>
                <Input
                  id="cfg-max-tokens"
                  type="number"
                  min={1}
                  value={config.max_tokens ?? ""}
                  onChange={(e) =>
                    setConfig({ ...config, max_tokens: asInt(e.target.value) })
                  }
                  data-testid="cfg-max-tokens"
                />
              </div>
              <div className="models-tab__field">
                <Label htmlFor="cfg-tps">tokens_per_second</Label>
                <Input
                  id="cfg-tps"
                  type="number"
                  min={0}
                  value={config.tokens_per_second ?? ""}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      tokens_per_second: asInt(e.target.value),
                    })
                  }
                  data-testid="cfg-tps"
                />
              </div>
              <div className="models-tab__field">
                <Label htmlFor="cfg-stop">stop_sequences</Label>
                <textarea
                  id="cfg-stop"
                  className="input"
                  rows={3}
                  placeholder="One per line"
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
                  data-testid="cfg-stop"
                />
                <span className="muted text-sm">One per line.</span>
              </div>
              <div className="models-tab__field">
                <Label htmlFor="cfg-system">system</Label>
                <textarea
                  id="cfg-system"
                  className="input"
                  rows={4}
                  placeholder="System prompt for this model"
                  value={config.system_prompt ?? ""}
                  onChange={(e) =>
                    setConfig({ ...config, system_prompt: e.target.value })
                  }
                  data-testid="cfg-system"
                />
              </div>
            </Stack>
          )}

          <Row justify="end" gap="sm">
            <Button
              variant="primary"
              disabled={loading || saving || !model}
              onClick={() => void handleSave()}
              data-testid="cfg-save"
            >
              {saving ? "Saving…" : "Save"}
            </Button>
          </Row>
        </>
      )}
    </Card>
  );
}
