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

import type { CSSProperties } from "react";
import { Spinner } from "../atoms/Spinner";
import { IconAlert, IconClose, IconInfo, IconWarning } from "../Icons";
import {
  MAX_VISUAL_DEPTH,
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

function NotificationRow({ n, depth }: { n: Notification; depth: number }) {
  const { dismiss } = useNotifications();
  const close = `Cerrar aviso: ${n.message}`;
  const front = depth === 0;

  return (
    <div
      className={[
        "notification",
        `notification--${n.kind}`,
        // Marca de "no es el de al frente". El CSS esconde el contenido de
        // los de atras para que asome solo la franja de color: con el texto
        // visible la franjita caia a media linea y se leia como texto
        // cortado.
        front ? "notification--front" : "notification--behind",
      ].join(" ")}
      // La profundidad la calcula el padre y se pasa por custom property en
      // vez de deducirla con `nth-last-child` en CSS: queda explicita,
      // testeable, y el CSS solo la consume.
      style={{ "--depth": depth } as CSSProperties}
      role={roleFor(n.kind)}
    >
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
      {/* El DOM va en orden de llegada (viejo -> nuevo) y el CSS pone el
          ultimo al frente con `z-index`. La profundidad se cuenta desde
          atras: el ultimo es 0, el anterior 1, y asi.
          `MAX_VISUAL_DEPTH` acota el escalonado, no la cantidad: los
          avisos siguen todos en el DOM y se cierran de a uno. */}
      {items.map((n, i) => (
        <NotificationRow
          key={n.id}
          n={n}
          depth={Math.min(items.length - 1 - i, MAX_VISUAL_DEPTH)}
        />
      ))}
    </div>
  );
}
