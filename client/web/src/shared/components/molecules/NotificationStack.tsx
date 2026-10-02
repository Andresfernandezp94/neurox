// NotificationStack — avisos del admin, superpuestos y centrados arriba.
//
// Un SOLO color para los cuatro kinds (tokens `--notification-*`), al
// estilo de la consola de AWS: la severidad la comunican el icono y la
// etiqueta, no el tinte. Con varias apiladas, un color por kind se leia
// como tres cosas sin relacion.
//
// Va montado DENTRO de `.app-main`, no sobre el viewport: asi se centra
// en el area de contenido y se reacomoda solo cuando la sidebar se
// contrae o se expande, sin calcular ningun offset contra el ancho de la
// sidebar.

import { Spinner } from "../atoms/Spinner";
import { IconAlert, IconClose, IconInfo, IconWarning } from "../Icons";
import {
  useNotificationItems,
  useNotifications,
  type Notification,
  type NotificationKind,
} from "../../../store/NotificationsContext";

/** Icono por kind. Es lo unico que distingue la severidad en el color. */
function KindIcon({ kind }: { kind: NotificationKind }) {
  if (kind === "process") return <Spinner size="sm" label="" />;
  if (kind === "error") return <IconAlert />;
  if (kind === "alert") return <IconWarning />;
  return <IconInfo />;
}

/**
 * `role`: los kinds de error y alerta son `alert` (interrumpe al lector de
 * pantalla). El proceso y el info son `status` (espera su turno): un
 * spinner no debe cortar lo que el usuario esta leyendo.
 */
function roleFor(kind: NotificationKind): "alert" | "status" {
  return kind === "error" || kind === "alert" ? "alert" : "status";
}

function NotificationRow({ n }: { n: Notification }) {
  const { dismiss } = useNotifications();
  const close = `Cerrar aviso: ${n.message}`;

  return (
    <div className={`notification notification--${n.kind}`} role={roleFor(n.kind)}>
      <span className="notification__icon" aria-hidden="true">
        <KindIcon kind={n.kind} />
      </span>
      <span className="notification__content">{n.message}</span>
      <button
        type="button"
        className="notification__dismiss"
        onClick={() => dismiss(n.id)}
        aria-label={close}
        title={close}
      >
        <IconClose />
      </button>
    </div>
  );
}

export function NotificationStack() {
  const items = useNotificationItems();
  if (items.length === 0) return null;

  return (
    <div className="notification-stack" data-testid="notification-stack">
      {items.map((n) => (
        <NotificationRow key={n.id} n={n} />
      ))}
    </div>
  );
}
