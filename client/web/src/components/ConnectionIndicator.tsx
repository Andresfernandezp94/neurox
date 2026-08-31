// Indicador de conexión global. EP-0003-03.
// EP-0024: el indicador es un outline aplicado al avatar del usuario.
// El estado combinado (ok/degraded/offline) determina el color del
// outline. Si el daemon está conectado, el outline pulsa.
// El componente expone un `id` y un `data-combined` para que el
// contenedor (AppHeader) pueda aplicar el outline al avatar.

import { useConnectionState } from '../store/StoreContext';

type Combined = 'offline' | 'degraded' | 'ok';

function computeCombined(conn: ReturnType<typeof useConnectionState>): Combined {
  if (conn.ws === 'closed') return 'offline';
  if (conn.health === null && conn.ws === 'open') return 'degraded';
  if (conn.isZombie) return 'degraded';
  if (conn.latencyMs !== null && conn.latencyMs > 500) return 'degraded';
  if (conn.lastRetryAt !== null && Date.now() - conn.lastRetryAt < 30_000) {
    return 'degraded';
  }
  return 'ok';
}

export function ConnectionIndicator() {
  const conn = useConnectionState();
  const combined = computeCombined(conn);

  const tooltip = [
    `WS: ${conn.ws}${conn.isZombie ? ' (zombie)' : ''}`,
    conn.latencyMs !== null ? `Latency: ${conn.latencyMs}ms` : 'Latency: —',
    `Retries: ${conn.retriesTotal}`,
    `Health: ${conn.health?.status ?? 'unknown'}`,
  ].join('\n');

  // No render DOM — el AppHeader lee `data-combined` del avatar
  // contenedor para aplicar el outline. Devolvemos un span invisible
  // solo para que React mantenga el árbol (sin esto no se monta
  // nada y el `useConnectionState` se suscribe inútilmente).
  return (
    <span
      className="conn-state-source"
      data-testid="connection-indicator"
      data-combined={combined}
      title={tooltip}
      aria-hidden="true"
      style={{ display: "none" }}
    />
  );
}