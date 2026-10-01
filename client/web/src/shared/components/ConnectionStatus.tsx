// ConnectionStatus — indicador de conexión con el daemon.
//
// Reemplaza al `<Badge dot>` que usaba el Overview. El punto verde
// pulsante era el recurso visual más genérico de "AI dashboard" que hay:
// cualquier template lo tiene. Este usa el hexágono del logo de neurox
// como glifo, con un anillo que se expande — nods de una red, no un
// punto.
//
// Tres estados, distinguibles sin depender solo del color:
//   live      hexágono + anillo expandiéndose (accent)
//   connecting hexágono con pulso lento, atenuado
//   offline   hexágono vacío, apagado, sin animación
//
// El texto acompaña siempre, así el estado no depende de decodificar
// una animación. `aria-live="polite"` para que un lector de pantalla
// anuncie los cambios de conexión.

import type { ConnectionState } from "../../store/StoreContext";

export type ConnectionKind = "live" | "connecting" | "offline";

export interface ConnectionStatusProps {
  conn: Pick<ConnectionState, "ws" | "health" | "isZombie">;
  className?: string;
}

/** Deriva el estado visual desde la conexión. */
export function connectionKind(
  conn: Pick<ConnectionState, "ws" | "health" | "isZombie">,
): ConnectionKind {
  if (conn.ws === "closed") return "offline";
  if (conn.health === null || conn.ws === "connecting") return "connecting";
  // Zombie = el WS está abierto pero el heartbeat no llega: la conexión
  // existe pero no sirve. Se degrada a "connecting", no a "live".
  if (conn.isZombie) return "connecting";
  return "live";
}

const LABEL: Record<ConnectionKind, string> = {
  live: "en línea",
  connecting: "conectando",
  offline: "sin conexión",
};

export function ConnectionStatus({ conn, className = "" }: ConnectionStatusProps) {
  const kind = connectionKind(conn);

  return (
    <span
      className={`conn-status conn-status--${kind} ${className}`.trim()}
      role="status"
      aria-live="polite"
      data-testid="connection-status"
      data-kind={kind}
    >
      <span className="conn-status__glyph" aria-hidden="true">
        {/* Anillo que se expande: sólo en live. */}
        {kind === "live" && <span className="conn-status__ping" />}
        <svg viewBox="0 0 24 24" className="conn-status__hex" focusable="false">
          <path
            d="M12 3.2 19 7v10l-7 3.8L5 17V7l7-3.8z"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            strokeLinejoin="round"
          />
          <circle cx="12" cy="12" r="2.4" fill="currentColor" />
        </svg>
      </span>
      <span className="conn-status__label">{LABEL[kind]}</span>
    </span>
  );
}