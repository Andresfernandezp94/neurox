// EnvTab — catalogo de variables de entorno del daemon. EP-0020-02.
//
// Los valores NUNCA se devuelven por API (solo `set`), asi que cada input
// arranca vacio y el placeholder dice si la variable ya esta seteada.
//
// Las API keys de providers NO se editan aca: cada provider declara su
// `api_key_env` y se edita en la tab Providers. Aparecen listadas en modo
// solo lectura, con un link a esa tab, para que el operador sepa que
// existen y donde viven sin tener dos lugares de escritura que se pisen.
//
// Las variables de infraestructura (HOME, PATH, XDG_*) tambien son solo
// lectura: las pone el host y escribir una ganaria hasta el proximo
// reinicio, y despues perderia contra el valor real sin avisar.
//
// Orden de las categorias segun las devuelve el daemon, no hardcodeado aca:
// si el catalogo gana una categoria nueva, aparece sola.

import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Input } from "../shared/components/atoms/Input";
import { IconButton } from "../shared/components/atoms/IconButton";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { Badge } from "../shared/components/atoms/Badge";
import {
  IconCpu,
  IconPlug,
  IconShield,
  IconCode,
  IconGlobe,
} from "../shared/components/Icons";
import {
  getEnv,
  putEnvVar,
  deleteEnvVar,
  type EnvVar,
  type EnvResponse,
} from "../api/env";

/** Icono por categoría. El fallback cubre categorías nuevas que el daemon
 *  agregue después: sin icono, pero visible. */
const CATEGORY_ICON: Record<string, React.ReactNode> = {
  runtime: <IconCpu />,
  "default-llm": <IconPlug />,
  auth: <IconShield />,
  integrations: <IconCode />,
  infrastructure: <IconGlobe />,
  custom: <IconCode />,
};

/** Texto de ayuda por variable que necesita más contexto del que cabe en
 *  una línea. El resto usa el `description` del daemon. */
const EXTRA_HINT: Record<string, string> = {
  NEUROX_ADMIN_PASSWORD:
    "Solo se lee al crear el usuario admin inicial, si no existe users.json. Después el cambio no aplica: usá la tab Users.",
  NEUROX_DEFAULT_PROVIDER:
    "Aplica solo si el usuario no eligió provider desde el selector. Si lo eligió, su elección gana.",
  NEUROX_WORKSPACE:
    "Reinicia el daemon para que tome efecto.",
  NEUROX_ENV_FILE:
    "Reinicia el daemon para que tome efecto. Afecta dónde se leen y escriben TODAS las variables.",
  NEUROX_MAX_TOOL_ITERATIONS:
    "Aplica al próximo subproceso de sesión.",
  NEUROX_MODELS_DIR:
    "Reinicia el daemon para que tome efecto.",
  NEUROX_LOG_FORMAT:
    "Reinicia el daemon para que tome efecto.",
};

export function EnvTab() {
  const [data, setData] = useState<EnvResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await getEnv());
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
      if (value === undefined || value === "") return;
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

  /** Agrupa por categoría respetando el orden del daemon. `unknownVars` se
   *  suma al final, bajo "Personalizadas". */
  const groups = useMemo(() => {
    if (!data) return [] as { id: string; label: string; description: string; items: EnvVar[] }[];
    const order = data.categories.map((c) => c.id);
    const byId = new Map<string, EnvVar[]>();
    for (const v of [...data.vars, ...data.unknownVars]) {
      const arr = byId.get(v.category) ?? [];
      arr.push(v);
      byId.set(v.category, arr);
    }
    // Categorías con items, en el orden que declara el daemon. Cualquier
    // categoría no declarada va al final, para no perderla.
    const ids = [
      ...order.filter((id) => byId.has(id)),
      ...Array.from(byId.keys()).filter((id) => !order.includes(id)),
    ];
    return ids
      .filter((id) => (byId.get(id) ?? []).length > 0)
      .map((id) => {
        const meta = data.categories.find((c) => c.id === id);
        return {
          id,
          label: meta?.label ?? id,
          description: meta?.description ?? "",
          items: byId.get(id) ?? [],
        };
      });
  }, [data]);

  const editableCount = useMemo(
    () => (data ? [...data.vars, ...data.unknownVars].filter((v) => !v.readOnly).length : 0),
    [data],
  );

  return (
    <div className="providers-list" data-testid="config-env-tab">
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <p className="muted text-sm providers-panel__description">
        Variables de entorno del daemon. Los valores nunca se devuelven: solo si
        están seteadas. Las API keys de providers se editan en la tab Providers.
      </p>

      {data && (
        <p className="muted text-sm providers-panel__description">
          <code>{data.path}</code> · {editableCount} editables
        </p>
      )}

      {loading ? (
        <p className="muted">Loading env vars…</p>
      ) : groups.length === 0 ? (
        <EmptyState>
          <EmptyState.Title>No env vars</EmptyState.Title>
          <EmptyState.Hint>The daemon reported no variables.</EmptyState.Hint>
        </EmptyState>
      ) : (
        groups.map((group) => (
          <Fragment key={group.id}>
            <div className="providers-list__section-head">
              <h4 className="muted">
                <span className="env-tab__category-icon">
                  {CATEGORY_ICON[group.id] ?? <IconCode />}
                </span>
                {group.label}
              </h4>
              <span className="muted text-sm">{group.items.length}</span>
            </div>

            {group.description && (
              <p className="muted text-sm providers-panel__description">
                {group.description}
              </p>
            )}

            {group.items.map((v) => {
              const hint = EXTRA_HINT[v.key];
              const dirty = drafts[v.key] !== undefined && drafts[v.key] !== "";
              return (
                <Card key={v.key} className="provider-card">
                  <Row justify="between" align="center" gap="sm">
                    <Row gap="sm" align="center">
                      <strong className="strong">{v.key}</strong>
                      {v.sensitive && (
                        <Badge variant="neutral" className="badge--active">
                          secret
                        </Badge>
                      )}
                      {v.readOnly && (
                        <Badge variant="neutral">solo lectura</Badge>
                      )}
                    </Row>
                    <span
                      className={`env-tab__state-badge env-tab__state-badge--${v.set ? "set" : "unset"}`}
                    >
                      {v.set ? "set" : "unset"}
                    </span>
                  </Row>

                  <p className="muted provider-endpoint">{v.description}</p>

                  {hint && <p className="muted text-sm">{hint}</p>}

                  {v.defaultValue !== null && !v.set && (
                    <p className="muted text-sm">
                      Default: <code>{v.defaultValue}</code>
                    </p>
                  )}

                  {v.readOnly ? (
                    <p className="muted text-sm">
                      {group.id === "default-llm"
                        ? "Se edita en la tab Providers."
                        : group.id === "infrastructure"
                          ? "La define el host. Neurox no la escribe."
                          : "No se edita desde acá."}
                    </p>
                  ) : (
                    <Row gap="sm" align="stretch" className="provider-keyrow">
                      <Input
                        type="password"
                        placeholder={v.set ? "•••••••• (set — escribí para reemplazar)" : "(unset)"}
                        value={drafts[v.key] ?? ""}
                        onChange={(e) =>
                          setDrafts((d) => ({ ...d, [v.key]: e.target.value }))
                        }
                        onKeyDown={(e) => {
                          if (e.key === "Enter" && dirty) void handleSave(v.key);
                        }}
                        className="env-tab__input"
                        aria-label={v.key}
                        data-testid={`env-input-${v.key}`}
                      />
                      <IconButton
                        icon="IconSave"
                        aria-label={`Save ${v.key}`}
                        title="Save"
                        disabled={pending === v.key || !dirty}
                        onClick={() => void handleSave(v.key)}
                        data-testid={`env-save-${v.key}`}
                      />
                      {v.set && (
                        <IconButton
                          icon="IconTrash"
                          aria-label={`Clear ${v.key}`}
                          title="Clear"
                          variant="danger"
                          disabled={pending === v.key}
                          onClick={() => void handleClear(v.key)}
                          data-testid={`env-clear-${v.key}`}
                        />
                      )}
                    </Row>
                  )}
                </Card>
              );
            })}
          </Fragment>
        ))
      )}
    </div>
  );
}