// ModelSelector — EP-0017-03 + EP-0024.
// Dropdown single-select del modelo de la sesión. La barra muestra
// solo el modelo activo; click abre la lista de modelos disponibles
// (uno por provider). El usuario hace click en uno y se guarda.
// EP-0024: cada modelo es una card con tags (chat, tools, free, etc.).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getProviders,
  listModels,
  setSessionModel,
  type CatalogModel,
} from "../api/llm";
import { ProviderLogo } from "../shared/components/ProviderLogo";

export interface ModelSelection {
  provider_id: string;
  model: string;
}

export interface ModelSelectorProps {
  sessionId: string;
  currentModel?: ModelSelection | null;
  onChange?: (selection: ModelSelection) => void;
  disabled?: boolean;
}

function encodeValue(providerId: string, modelId: string): string {
  return `${providerId}::${modelId}`;
}

function decodeValue(value: string): ModelSelection | null {
  const idx = value.indexOf("::");
  if (idx < 0) return null;
  return { provider_id: value.slice(0, idx), model: value.slice(idx + 2) };
}

/** Parse a raw model id into a human-readable name.
 *  Examples:
 *    inclusionai/ling-3.0-tiny:free      → "ling-3.0-tiny:free"  → "ling 3.0 tiny:free"
 *    meta/muse-spark-1.2-20260805        → "muse-spark-1.2-20260805"  → "muse spark 1.2"
 *    mistral-large-latest               → "mistral-large-latest"  → "mistral large latest"
 *    qwen2.5-1.5b-instruct-q4_k_m.gguf  → "qwen 2.5 1.5b instruct q4 k m.gguf"
 *  Replaces `-` with spaces (so separators read as words), strips a
 *  trailing `-YYYYMMDD` date suffix and a `:free`/`:beta` suffix, and
 *  truncates at `max` chars with an ellipsis if longer.
 */
export function formatModelName(modelId: string, max = 36): string {
  let name = modelId;

  // Strip a `provider/` prefix (common in openrouter model ids).
  if (name.includes("/")) {
    name = name.slice(name.lastIndexOf("/") + 1);
  }

  // Strip a `:free` or `:beta` suffix.
  if (name.includes(":")) {
    name = name.slice(0, name.indexOf(":"));
  }

  // Strip a trailing `-YYYYMMDD` date suffix (openrouter sometimes adds it).
  if (name.length >= 9) {
    const tail = name.slice(-9);
    if (/^-\d{8}$/.test(tail)) {
      name = name.slice(0, -9);
    }
  }

  // Replace `-` with spaces for readability (keep `.`, `_`).
  name = name.replace(/-/g, " ");

  // Collapse multiple spaces.
  name = name.replace(/\s+/g, " ").trim();

  if (name.length > max) {
    name = name.slice(0, max - 1) + "…";
  }
  return name;
}

/** Map the daemon's kebab-case capability to a short Title Case label
 *  for the UI. Mirrors the backend's `ModelCapability::label()`.
 */
const CAPABILITY_LABELS: Record<string, string> = {
  "text-generation": "Text Generation",
  "chat": "Chat",
  "embeddings": "Embeddings",
  "image": "Image Gen",
  "vision": "Vision",
  "audio": "Audio",
  "tts": "TTS",
  "transcription": "Transcription",
  "code": "Code",
  "rerank": "Rerank",
  "unknown": "Unknown",
};

function humanCapability(c: string): string {
  return CAPABILITY_LABELS[c] ?? c;
}

export function ModelSelector({
  sessionId,
  currentModel,
  onChange,
  disabled,
}: ModelSelectorProps) {
  const [models, setModels] = useState<CatalogModel[]>([]);
  const [defaultProvider, setDefaultProvider] = useState<string>("");
  const [defaultModel, setDefaultModel] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [userSelection, setUserSelection] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  /** EP-0024: which provider groups are expanded in the dropdown. Empty
   *  by default — the operator clicks a provider header to reveal its
   *  models. Providers with matches under an active search are
   *  auto-expanded (see `visibleGroups`). */
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const wrapperRef = useRef<HTMLDivElement>(null);

  const toggleProvider = useCallback((providerId: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(providerId)) next.delete(providerId);
      else next.add(providerId);
      return next;
    });
  }, []);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const providersResp = await getProviders();
        // Discover models per configured provider (same strategy as
        // ProvidersPanel → handleRefreshModels). The global
        // /v1/llm/models catalog only returns models for one provider
        // (verified 2026-08-13), so we fetch per-provider and merge.
        const configured = providersResp.providers.filter((p) => p.configured);
        const perProvider = await Promise.all(
          configured.map(async (p) => {
            const discovered = await listModels(p.id);
            return discovered.map(
              (m) =>
                ({
                  provider_id: p.id,
                  model_id: m.id,
                  kind: p.kind,
                  base_url: p.base_url,
                  capability: m.capability,
                  supports_tools: (m as { supports_tools?: boolean }).supports_tools,
                  is_free: (m as { is_free?: boolean }).is_free,
                }) satisfies CatalogModel,
            );
          }),
        );
        setModels(perProvider.flat());
        setDefaultProvider(providersResp.default_provider);
        setDefaultModel(providersResp.default_model);
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setLoading(false);
      }
    };
    void load();
  }, []);

  const refreshCatalog = useCallback(async () => {
    setLoading(true);
    try {
      const providersResp = await getProviders();
      const configured = providersResp.providers.filter((p) => p.configured);
      const perProvider = await Promise.all(
        configured.map(async (p) => {
          const discovered = await listModels(p.id);
          return discovered.map(
            (m) =>
              ({
                provider_id: p.id,
                model_id: m.id,
                kind: p.kind,
                base_url: p.base_url,
                capability: m.capability,
                supports_tools: (m as { supports_tools?: boolean }).supports_tools,
                is_free: (m as { is_free?: boolean }).is_free,
              }) satisfies CatalogModel,
          );
        }),
      );
      setModels(perProvider.flat());
      setDefaultProvider(providersResp.default_provider);
      setDefaultModel(providersResp.default_model);
    } catch {
      // Silencioso: la carga inicial ya maneja errores visibles.
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) {
      void refreshCatalog();
    }
  }, [open, refreshCatalog]);

  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, [open]);

  const defaultSelection = useMemo(() => {
    if (defaultProvider && defaultModel) {
      return encodeValue(defaultProvider, defaultModel);
    }
    return "";
  }, [defaultProvider, defaultModel]);

  const selected = userSelection
    ?? (currentModel ? encodeValue(currentModel.provider_id, currentModel.model) : null)
    ?? (defaultSelection || null)
    ?? "";

  const grouped = useMemo(() => {
    const byProvider = new Map<string, CatalogModel[]>();
    for (const m of models) {
      // Defensive: drop malformed entries with no model_id so the
      // sort below never hits `undefined.localeCompare` (crashed the
      // whole selector when the daemon returned string-only models).
      if (!m || !m.model_id || !m.provider_id) continue;
      const arr = byProvider.get(m.provider_id) ?? [];
      arr.push(m);
      byProvider.set(m.provider_id, arr);
    }
    for (const arr of byProvider.values()) {
      arr.sort((a, b) => (a.model_id ?? "").localeCompare(b.model_id ?? ""));
    }
    return Array.from(byProvider.entries()).sort((a, b) =>
      (a[0] ?? "").localeCompare(b[0] ?? ""),
    );
  }, [models]);

  const handleSelect = useCallback(
    async (value: string) => {
      const parsed = decodeValue(value);
      if (!parsed) return;
      if (value === selected) return;
      setUserSelection(value);
      setSaving(true);
      setError(null);
      try {
        await setSessionModel(sessionId, parsed.provider_id, parsed.model);
        onChange?.(parsed);
        setOpen(false);
      } catch (err) {
        setUserSelection(null);
        setError((err as Error).message);
      } finally {
        setSaving(false);
      }
    },
    [sessionId, selected, onChange],
  );

  const activeModel = decodeValue(selected);
  const activeLabel = activeModel
    ? formatModelName(activeModel.model)
    : (loading ? "loading…" : "select model");

  return (
    <div className="model-selector" ref={wrapperRef}>
      <button
        type="button"
        className="model-selector__trigger"
        onClick={() => setOpen((o) => !o)}
        disabled={disabled || saving || loading}
        aria-haspopup="listbox"
        aria-expanded={open}
        data-testid="model-selector-trigger"
      >
        {activeModel && (
          <ProviderLogo
            id={activeModel.provider_id}
            kind={activeModel.provider_id === "local-llama" ? "openai_compat" : "openai_compat"}
            className="model-selector__trigger-logo"
          />
        )}
        <span className="model-selector__trigger-label">{activeLabel}</span>
        <span className={`model-selector__caret${open ? " model-selector__caret--open" : ""}`}>▾</span>
      </button>

      {open && (
        <div className="model-selector__dropdown" role="listbox" data-testid="model-selector-dropdown">
          <div className="model-selector__search">
            <span className="model-selector__search-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="11" cy="11" r="7" />
                <path d="M21 21l-4.3-4.3" />
              </svg>
            </span>
            <input
              type="text"
              className="model-selector__search-input"
              placeholder="search…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              autoFocus
              data-testid="model-selector-search"
            />
            <button
              type="button"
              className="model-selector__refresh"
              title="Refresh catalog"
              aria-label="Refresh catalog"
              onClick={() => void refreshCatalog()}
              data-testid="model-selector-refresh"
            >
              ↻
            </button>
          </div>
          {grouped
            .map(([providerId, modelsForProvider]) => {
              const filtered = modelsForProvider.filter((m) => {
                if (!filter) return true;
                const q = filter.toLowerCase();
                return (
                  m.model_id.toLowerCase().includes(q) ||
                  formatModelName(m.model_id).toLowerCase().includes(q) ||
                  providerId.toLowerCase().includes(q)
                );
              });
              // Auto-expand groups that have matches under an active search.
              const isExpanded = filter
                ? filtered.length > 0
                : expanded.has(providerId);
              return { providerId, modelsForProvider, filtered, isExpanded };
            })
            .filter((g) => g.filtered.length > 0)
            .map(({ providerId, modelsForProvider, filtered, isExpanded }) => (
              <div key={providerId} className="model-selector__group">
                <button
                  type="button"
                  className="model-selector__group-header"
                  onClick={() => toggleProvider(providerId)}
                  aria-expanded={isExpanded}
                  data-testid={`model-selector-group-${providerId}`}
                >
                  <ProviderLogo
                    id={providerId}
                    kind={modelsForProvider[0]?.kind ?? "openai_compat"}
                    className="model-selector__group-logo"
                  />
                  <span className="model-selector__group-label">{providerId}</span>
                  <span className="model-selector__group-count">{filtered.length}</span>
                  <span className={`model-selector__group-caret${isExpanded ? " model-selector__group-caret--open" : ""}`}>▾</span>
                </button>
                {isExpanded && (
                  <div className="model-selector__group-items">
                    {filtered.map((m) => {
                      const value = encodeValue(providerId, m.model_id);
                      const isActive = selected === value;
                      const display = formatModelName(m.model_id);
                      return (
                        <button
                          type="button"
                          key={value}
                          role="option"
                          aria-selected={isActive}
                          className={`model-selector__item${isActive ? " model-selector__item--active" : ""}`}
                          title={m.model_id}
                          data-testid={`model-selector-item-${value}`}
                          onClick={() => void handleSelect(value)}
                        >
                          <ProviderLogo
                            id={providerId}
                            kind={m.kind}
                            className="model-selector__item-logo"
                          />
                          <div className="model-selector__item-main">
                            <span className="model-selector__name">{display}</span>
                            <div className="model-selector__tags">
                              {m.capability && m.capability !== "unknown" && (
                                <span className={`model-tag model-tag--${m.capability}`}>
                                  {humanCapability(m.capability)}
                                </span>
                              )}
                              {m.supports_tools && (
                                <span className="model-tag model-tag--tools" title="Supports function calling">
                                  tools
                                </span>
                              )}
                              {m.is_free && (
                                <span className="model-tag model-tag--free" title="Free tier">
                                  free
                                </span>
                              )}
                              {providerId === "local-llama" && (
                                <span className="model-tag model-tag--local" title="Local model">
                                  local
                                </span>
                              )}
                            </div>
                          </div>
                          {isActive && <span className="model-selector__check">✓</span>}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            ))}
          {models.length === 0 && (
            <div className="model-selector__empty muted text-xs">no models available</div>
          )}
          {models.length > 0 && filter && grouped.every(([_, arr]) => {
            const q = filter.toLowerCase();
            return !arr.some((m) =>
              m.model_id.toLowerCase().includes(q) ||
              formatModelName(m.model_id).toLowerCase().includes(q)
            );
          }) && (
            <div className="model-selector__empty muted text-xs">no matches</div>
          )}
        </div>
      )}

      {error && <div className="model-selector__error">{error}</div>}
    </div>
  );
}