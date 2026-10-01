// LocalServiceCard — configuración y on/off del servicio LLM local.
//
// Vive en la tab Local y NO en Providers, a propósito: un provider local
// no tiene API key ni cuenta, y meterlo en la lista de providers remoto
// lo hace indistinguishable de algo que necesita un key. El lugar donde se
// configura es donde se ven los modelos GGUF que va a servir.
//
// El backend ya soporta todo esto: `POST/PUT /v1/llm/providers` acepta
// `local_command`, `local_args`, `local_model_path` y `local_port`, y
// `POST/DELETE /v1/llm/providers/:id/{start,stop}` lo arranca y lo para a
// través del orquestador. Acá no se inventa nada: se cablea.
//
// ## llama-server
//
// Backend por defecto. `llama-server` expone `/health` (que es lo que el
// orquestador sondea para dar el servicio por listo) y una API
// OpenAI-compatible en `/v1`, así que el resto de neurox lo trata como un
// provider `openai_compat` sin ningún caso especial.
//
// Los placeholders `{{port}}` y `{{model_path}}` los expande el daemon. El
// puerto es obligatorio: sin él el orquestador no tiene contra qué sondear
// la readiness.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Input } from "../shared/components/atoms/Input";
import { Label } from "../shared/components/atoms/Label";
import { Button } from "../shared/components/atoms/Button";
import { Badge } from "../shared/components/atoms/Badge";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { ProviderLogo } from "../shared/components/ProviderLogo";
import { IconCheck, IconPause, IconPlay, IconTrash } from "../shared/components/Icons";
import {
  getProviders,
  addProvider,
  updateProvider,
  startProvider,
  stopProvider,
  deleteProvider,
  type LlmProviderStatus,
  type ServiceState,
} from "../api/llm";

/** Id del provider local que crea esta card. Fijo para que la tab tenga un
 *  solo servicio local: varios llama-server compitiendo por GPU no es un
 *  caso de uso, y multiplies la card sin necesidad. */
export const LOCAL_SERVICE_ID = "local-llama-server";

const DEFAULT_PORT = 8080;
const DEFAULT_COMMAND = "llama-server";

/** Args de llama-server con los placeholders del orquestador.
 *  `--host 127.0.0.1` es obligatorio en la práctica: llama-server escucha en
 *  0.0.0.0 por defecto, y exponer un modelo sin auth en la red local no
 *  es algo que deba pasar por defecto. */
function defaultArgs(): string[] {
  return [
    "--model",
    "{{model_path}}",
    "--port",
    "{{port}}",
    "--host",
    "127.0.0.1",
  ];
}

export interface LocalServiceCardProps {
  /** GGUF disponibles, ya categorizados por el daemon. Solo se ofrecen los
   *  de categoría chat: un embedding no se sirve con llama-server. */
  chatModels: { filename: string; path: string }[];
  /** Cambia cuando el provider se crea o se borra, para que la tab lo
   *  recargue. */
  onChanged?: () => void;
}

export function LocalServiceCard({ chatModels, onChanged }: LocalServiceCardProps) {
  const [service, setService] = useState<LlmProviderStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Borrador de configuración. Solo se usa cuando NO existe service.
  const [modelPath, setModelPath] = useState("");
  const [command, setCommand] = useState(DEFAULT_COMMAND);
  const [port, setPort] = useState(String(DEFAULT_PORT));

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const resp = await getProviders();
      const found = resp.providers.find((p) => p.id === LOCAL_SERVICE_ID) ?? null;
      setService(found);
      // Si ya existe, el formulario refleja su configuración real: el
      // operador tiene que ver lo que está corriendo, no un default.
      if (found?.local_model_path) setModelPath(found.local_model_path);
      if (found?.local_command) setCommand(found.local_command);
      if (found?.local_port) setPort(String(found.local_port));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const svcState = (service?.service_state ?? "stopped") as ServiceState;
  const isRunning = svcState === "running" || svcState === "ready";
  const isStarting = svcState === "starting";
  const isFailed = svcState === "failed";

  const portNum = Number(port);
  const portValid = Number.isInteger(portNum) && portNum > 0 && portNum < 65536;
  const canCreate = command.trim() !== "" && portValid && modelPath !== "";

  /** Crea el provider local. El modelo elegido va en `local_model_path` y
   *  `{{model_path}}` lo expande el daemon al spawnear. */
  const handleCreate = useCallback(async () => {
    if (!canCreate) return;
    setBusy(true);
    setError(null);
    try {
      await addProvider({
        id: LOCAL_SERVICE_ID,
        kind: "openai_compat",
        // llama-server expone una API OpenAI-compatible: el resto de neurox
        // lo trata como cualquier provider openai_compat.
        base_url: `http://127.0.0.1:${portNum}/v1`,
        model: modelPath.split("/").pop() ?? modelPath,
        local_command: command.trim(),
        local_args: defaultArgs(),
        local_model_path: modelPath,
        local_port: portNum,
      });
      await load();
      onChanged?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, [canCreate, command, modelPath, portNum, load, onChanged]);

  const handleSave = useCallback(async () => {
    if (!service || !canCreate) return;
    setBusy(true);
    setError(null);
    try {
      await updateProvider(service.id, {
        local_command: command.trim(),
        local_model_path: modelPath,
        local_port: portNum,
        base_url: `http://127.0.0.1:${portNum}/v1`,
      });
      await load();
      onChanged?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, [service, canCreate, command, modelPath, portNum, load, onChanged]);

  const handleToggle = useCallback(async () => {
    if (!service) return;
    setBusy(true);
    setError(null);
    try {
      if (isRunning) {
        await stopProvider(service.id);
      } else {
        await startProvider(service.id);
      }
      await load();
      onChanged?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, [service, isRunning, load, onChanged]);

  const handleDelete = useCallback(async () => {
    if (!service) return;
    setBusy(true);
    setError(null);
    try {
      await stopProvider(service.id).catch(() => {});
      await deleteProvider(service.id);
      setService(null);
      setModelPath("");
      setCommand(DEFAULT_COMMAND);
      setPort(String(DEFAULT_PORT));
      onChanged?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, [service, onChanged]);

  const chatList = useMemo(() => chatModels, [chatModels]);

  return (
    <Card
      className={`provider-card ${service ? "provider-card--active" : ""}`}
      data-testid="local-service-card"
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <header className="provider-card__header">
        <div className="provider-card__identity">
          <ProviderLogo id={LOCAL_SERVICE_ID} kind="local" className="provider-logo" />
          <strong className="strong">Local service</strong>
          {service && (
            <Badge
              className={isFailed ? "badge--danger" : isRunning ? "badge--success" : "badge--muted"}
              data-testid="local-service-state"
            >
              {isStarting ? "starting" : svcState}
            </Badge>
          )}
        </div>
        <div className="provider-card__actions">
          {service && (
            <button
              type="button"
              disabled={busy || isStarting}
              className={`provider-action ${isRunning ? "provider-action--delete" : "provider-action--save"}`}
              title={isRunning ? "Stop local service" : "Start local service"}
              data-testid="local-svc-toggle"
              onClick={() => void handleToggle()}
            >
              {isRunning ? <IconPause /> : <IconPlay />}
            </button>
          )}
          {service && (
            <button
              type="button"
              disabled={busy}
              className="provider-action provider-action--delete"
              title="Remove local service"
              data-testid="local-svc-delete"
              onClick={() => void handleDelete()}
            >
              <IconTrash />
            </button>
          )}
        </div>
      </header>

      <hr className="provider-card__divider" />

      {loading ? (
        <p className="muted text-sm">Loading local service…</p>
      ) : !service ? (
        /* ─── Sin configurar: el formulario de creación ─── */
        <>
          <p className="muted text-sm">
            Corre un servidor LLM local sobre un GGUF de la categoría{" "}
            <code>chat</code>. No necesita API key: el modelo se sirve desde tu
            máquina.
          </p>

          {chatList.length === 0 ? (
            <p className="muted text-sm" data-testid="local-svc-no-chat-models">
              No hay modelos de chat en el directorio. Configuralo primero en la
              sección de abajo, o usá un GGUF de otra categoría.
            </p>
          ) : (
            <div className="local-service__field">
              <Label htmlFor="local-svc-model">Modelo</Label>
              <select
                id="local-svc-model"
                className="input"
                value={modelPath}
                onChange={(e) => setModelPath(e.target.value)}
                data-testid="local-svc-model"
              >
                <option value="">Elegí un modelo…</option>
                {chatList.map((m) => (
                  <option key={m.path} value={m.path}>
                    {m.filename}
                  </option>
                ))}
              </select>
            </div>
          )}

          <div className="local-service__field">
            <Label htmlFor="local-svc-command">Binario</Label>
            <Input
              id="local-svc-command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder={DEFAULT_COMMAND}
              data-testid="local-svc-command"
            />
            <p className="muted text-sm">
              Ruta absoluta si no está en tu PATH. Bajo systemd el PATH es
              mínimo.
            </p>
          </div>

          <div className="local-service__field">
            <Label htmlFor="local-svc-port">Puerto</Label>
            <Input
              id="local-svc-port"
              type="number"
              value={port}
              onChange={(e) => setPort(e.target.value)}
              data-testid="local-svc-port"
            />
            {!portValid && port !== "" && (
              <p className="local-service__invalid" role="alert">
                Puerto inválido: tiene que ser un número entre 1 y 65535.
              </p>
            )}
          </div>

          <Button
            onClick={() => void handleCreate()}
            disabled={busy || !canCreate}
            data-testid="local-svc-create"
          >
            <IconCheck /> Configurar
          </Button>
        </>
      ) : (
        /* ─── Configurado: estado + edición ─── */
        <>
          <div className="provider-endpoint">
            {service.base_url || `http://127.0.0.1:${service.local_port}/v1`}
          </div>

          {isRunning ? (
            <p className="muted text-sm">
              Sirviendo <code>{service.local_model_path?.split("/").pop()}</code>.
              Quedá accesible para el chat como cualquier provider.
            </p>
          ) : (
            <p className="muted text-sm">
              El servicio está detenido. Configurá el modelo y el puerto, y
              después arrancalo.
            </p>
          )}

          {isFailed && (
            <p className="local-service__invalid" role="alert">
              El servicio falló al arrancar. Revisá que el GGUF exista y que
              el puerto esté libre.
            </p>
          )}

          <div className="local-service__field">
            <Label htmlFor="local-svc-command">Binario</Label>
            <Input
              id="local-svc-command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              data-testid="local-svc-command"
            />
          </div>

          <div className="local-service__field">
            <Label htmlFor="local-svc-port">Puerto</Label>
            <Input
              id="local-svc-port"
              type="number"
              value={port}
              onChange={(e) => setPort(e.target.value)}
              data-testid="local-svc-port"
            />
          </div>

          <div className="local-service__field">
            <Label htmlFor="local-svc-model">Modelo</Label>
            <select
              id="local-svc-model"
              className="input"
              value={modelPath}
              onChange={(e) => setModelPath(e.target.value)}
              data-testid="local-svc-model"
            >
              {chatList.map((m) => (
                <option key={m.path} value={m.path}>
                  {m.filename}
                </option>
              ))}
              {/* El modelo configurado puede no estar en la lista (borrado,
                  o de otra categoría). Sin esta option, el select caería a
                  la primera opción y el operador creería que cambió. */}
              {!chatList.some((m) => m.path === modelPath) && modelPath !== "" && (
                <option value={modelPath}>
                  {modelPath.split("/").pop()} (no encontrado)
                </option>
              )}
            </select>
          </div>

          <Button
            onClick={() => void handleSave()}
            disabled={busy || !canCreate}
            data-testid="local-svc-save"
          >
            <IconCheck /> Guardar
          </Button>
        </>
      )}
    </Card>
  );
}