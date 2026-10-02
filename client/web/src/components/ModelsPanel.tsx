// ModelsPanel — Local GGUF + Hugging Face search + Configure modal. EP-0020-02.

import { useCallback, useEffect, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { Input } from "../shared/components/atoms/Input";
import { Label } from "../shared/components/atoms/Label";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { SearchBar } from "../shared/components/molecules/SearchBar";
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

type Tab = "local" | "hf";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function ModelsPanel() {
  const [tab, setTab] = useState<Tab>("local");
  const [local, setLocal] = useState<LocalModel[]>([]);
  // `null` cuando MODELS_DIR no existe. Antes se tipaba `string` con `""` de
  // default, pero el daemon devuelve `null` si el directorio no esta: el
  // tipo mentia y el primer consumidor que lo leyera se crasharia.
  const [dir, setDir] = useState<string | null>(null);
  const [hf, setHf] = useState<HfModel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [searching, setSearching] = useState(false);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [editing, setEditing] = useState<LocalModel | null>(null);

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

  useEffect(() => {
    void load();
  }, [load]);

  const doSearch = useCallback(async () => {
    if (!search.trim()) return;
    setSearching(true);
    setError(null);
    try {
      const data = await searchHfModels(search.trim());
      setHf(data.models);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSearching(false);
    }
  }, [search]);

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
    <Stack gap="md">
      <h3 className="models-section__title">Models</h3>
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Row gap="sm">
        <Button
          variant={tab === "local" ? "primary" : "secondary"}
          onClick={() => setTab("local")}
        >
          Local ({local.length})
        </Button>
        <Button
          variant={tab === "hf" ? "primary" : "secondary"}
          onClick={() => setTab("hf")}
        >
          Hugging Face
        </Button>
      </Row>

      {tab === "local" && (
        <>
          <Label className="muted">
            dir: <code>{dir || "?"}</code>
          </Label>
          {loading && <p className="muted">Loading local models…</p>}
          {!loading && local.length === 0 && (
            <EmptyState>
              <EmptyState.Title>No local models</EmptyState.Title>
              <EmptyState.Hint>
                The MODELS_DIR is empty. Download a model from Hugging Face.
              </EmptyState.Hint>
            </EmptyState>
          )}
          {local.map((m) => (
            <Card key={m.filename} className="models-panel__local-card">
              <Row justify="between" align="center">
                <Row gap="md">
                  <strong className="models-panel__filename">{m.filename}</strong>
                  <span className="muted">{formatSize(m.size_bytes)}</span>
                </Row>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setEditing(m)}
                >
                  Configure
                </Button>
              </Row>
            </Card>
          ))}
        </>
      )}

      {tab === "hf" && (
        <>
          <SearchBar
            placeholder="Search Hugging Face (e.g. qwen2.5)"
            value={search}
            onChange={setSearch}
          />
          <Button
            variant="primary"
            size="sm"
            disabled={searching || !search.trim()}
            onClick={doSearch}
          >
            {searching ? "Searching…" : "Search"}
          </Button>
          {searching && <p className="muted">Searching…</p>}
          {!searching && hf.length === 0 && search && (
            <EmptyState>
              <EmptyState.Title>No results</EmptyState.Title>
              <EmptyState.Hint>Try a different query.</EmptyState.Hint>
            </EmptyState>
          )}
          {hf.map((m) => (
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
        </>
      )}

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
              <Label>system prompt</Label>
              <textarea
                className="input"
                rows={3}
                value={config.system_prompt ?? ""}
                onChange={(e) =>
                  setConfig({ ...config, system_prompt: e.target.value })
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
