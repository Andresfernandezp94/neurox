// Store de notificaciones del admin.
//
// Tres kinds, un solo color (ver los tokens `--notification-*`): la
// severidad la comunica el icono y la etiqueta, no el tinte. Con varias
// apiladas, un color por kind se leia como tres cosas sin relacion.
//
// Ciclo de vida de un `process`, que es lo pedido: se ve mientras corre,
// desaparece al completarse bien, y si falla se convierte en `error` y
// espera a que el usuario lo cierre. No hay toast de exito: el exito se
// comunica con el resultado que la propia operacion produce (un mensaje
// en el chat, una fila en la tabla), y un toast extra seria ruido.
//
// Es un store aparte y no parte de `StoreContext` porque ese ya es el
// estado del daemon (agentes, sesiones, aprobaciones, conexion). Las
// notificaciones son efimeras y locales a la vista: no van al WS ni se
// persisten.

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

export type NotificationKind = "process" | "error" | "alert" | "info";

export interface Notification {
  id: number;
  kind: NotificationKind;
  message: string;
}

/**
 * Tope de notificaciones visibles a la vez. Pasado el tope cae la mas
 * vieja: las nuevas pesan mas que una de hace rato que el usuario todavia
 * no leyo, y el stack no puede terminar tapando la pantalla.
 */
export const MAX_VISIBLE = 5;

export interface NotificationsApi {
  /** Agrega una notificacion y devuelve su id. */
  notify: (kind: NotificationKind, message: string) => number;
  error: (message: string) => number;
  alert: (message: string) => number;
  info: (message: string) => number;
  dismiss: (id: number) => void;
  clear: () => void;
  /** Agrega un proceso en curso. Visible hasta que se resuelva o falle. */
  startProcess: (message: string) => number;
  /** El proceso termino bien: desaparece. */
  resolveProcess: (id: number) => void;
  /** El proceso fallo: pasa a `error` y queda esperando la X. */
  failProcess: (id: number, message: string) => void;
  /**
   * Envoltura para lo que quiere la mayoria de los call sites: publica el
   * proceso, corre la operacion y resuelve o falla sola.
   *
   * Vuelve a lanzar el error: el que llama decide que hacer con el ademas
   * de la notificacion, y casi siempre tambien quiere limpiar su estado
   * local (deshabilitar un boton, restaurar un campo).
   */
  runProcess: <T>(message: string, fn: () => Promise<T>) => Promise<T>;
}

interface NotificationsContextValue {
  items: Notification[];
  api: NotificationsApi;
}

const NotificationsContext = createContext<NotificationsContextValue | null>(null);

/** Recorta la lista al tope, tirando lo mas viejo. */
function cap(list: Notification[]): Notification[] {
  return list.length > MAX_VISIBLE ? list.slice(list.length - MAX_VISIBLE) : list;
}

export function NotificationsProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Notification[]>([]);
  const seq = useRef(0);

  const dismiss = useCallback((id: number) => {
    setItems((prev) => prev.filter((n) => n.id !== id));
  }, []);

  const clear = useCallback(() => setItems([]), []);

  const notify = useCallback((kind: NotificationKind, message: string): number => {
    const text = message.trim();
    // Id 0 = "no se publico nada". Un mensaje vacio no tiene nada que
    // mostrar, y devolver un id valido invitaria al que llama a resolver
    // o fallar una notificacion que no existe.
    if (!text) return 0;

    seq.current += 1;
    const id = seq.current;
    // Sin agrupar: cada aviso entra como fila propia aunque el mensaje sea
    // identico al de arriba. Agruparlos con un contador ("x3") se leia
    // como un resumen, y lo que se quiere ver son los avisos uno debajo
    // del otro, que es como se entienden los que importan.
    setItems((prev) => cap([...prev, { id, kind, message: text }]));
    return id;
  }, []);

  const startProcess = useCallback((message: string) => notify("process", message), [notify]);

  const resolveProcess = useCallback(
    (id: number) => {
      if (id === 0) return;
      dismiss(id);
    },
    [dismiss],
  );

  const failProcess = useCallback(
    (id: number, message: string) => {
      if (id === 0) {
        // No hay proceso al que pegarle el error (mensaje de arranque
        // vacio, o nunca se publico). Va como error suelto.
        notify("error", message);
        return;
      }
      const text = message.trim();
      if (!text) {
        // Fallo sin detalle: sacarlo es mejor que dejar un proceso
        // girando para siempre.
        dismiss(id);
        return;
      }
      setItems((prev) =>
        cap(
          prev.map((n) =>
            n.id === id ? { ...n, kind: "error" as const, message: text } : n,
          ),
        ),
      );
    },
    [notify, dismiss],
  );

  const runProcess = useCallback(
    async <T,>(message: string, fn: () => Promise<T>): Promise<T> => {
      const id = startProcess(message);
      try {
        const out = await fn();
        resolveProcess(id);
        return out;
      } catch (e) {
        failProcess(id, e instanceof Error ? e.message : String(e));
        throw e;
      }
    },
    [startProcess, resolveProcess, failProcess],
  );

  const api = useMemo<NotificationsApi>(
    () => ({
      notify,
      error: (m: string) => notify("error", m),
      alert: (m: string) => notify("alert", m),
      info: (m: string) => notify("info", m),
      dismiss,
      clear,
      startProcess,
      resolveProcess,
      failProcess,
      runProcess,
    }),
    [notify, dismiss, clear, startProcess, resolveProcess, failProcess, runProcess],
  );

  const value = useMemo<NotificationsContextValue>(
    () => ({ items, api }),
    [items, api],
  );

  return (
    <NotificationsContext.Provider value={value}>{children}</NotificationsContext.Provider>
  );
}

export function useNotifications(): NotificationsApi {
  const ctx = useContext(NotificationsContext);
  if (!ctx) {
    throw new Error("useNotifications must be used within a NotificationsProvider");
  }
  return ctx.api;
}

/** Las notificaciones vivas. Lo consume el stack. */
export function useNotificationItems(): Notification[] {
  const ctx = useContext(NotificationsContext);
  if (!ctx) {
    throw new Error("useNotificationItems must be used within a NotificationsProvider");
  }
  return ctx.items;
}
