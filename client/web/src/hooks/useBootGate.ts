// useBootGate.ts — decide cuándo se deja de mostrar el BootLoader.
//
// Por qué existe: `App.tsx` usa `state.loaded.health` para decidir si
// mostrar el loader. Ese flag SOLO pasa a `true` cuando el `GET /health`
// resuelve (`SNAPSHOT_HEALTH` se despacha únicamente con
// `healthR.status === 'fulfilled'`). Si el daemon está caído, la promesa
// rechaza, el flag nunca cambia y el loader queda pegado para siempre:
// la app es inalcanzable.
//
// Este gate resuelve el problema sin tocar la lógica de red:
//
//   1. Mínimo de duración — el loader no parpadea. Un arranque instantáneo
//      se lee como un glitch, no como una transición.
//   2. Timeout máximo — pase lo que pase con el daemon, a los `maxMs`
//      se revela la app. El estado de conexión se muestra en la propia
//      UI (dot / badge), que es donde corresponde.
//
// En modo mock el gate no espera nada: el store ya está hidratado.

import { useEffect, useState } from "react";
import { isMockMode } from "../shared/mock/mockData";

export interface BootGateOptions {
  /** El store terminó de hidratar health. */
  ready: boolean;
  /** Duración mínima del loader, en ms. Evita el flash. */
  minMs?: number;
  /** Tope duro. A partir de acá se revela la app igual. */
  maxMs?: number;
}

export interface BootGate {
  /** true mientras corresponde mostrar el loader. */
  booting: boolean;
}

const DEFAULT_MIN_MS = 700;
const DEFAULT_MAX_MS = 4_000;

export function useBootGate({
  ready,
  minMs = DEFAULT_MIN_MS,
  maxMs = DEFAULT_MAX_MS,
}: BootGateOptions): BootGate {
  const [elapsed, setElapsed] = useState(0);

  // Reloj de arranque. Se desmonta apenas se cumple la condición de
  // salida, así no queda un timer colgando en background.
  useEffect(() => {
    const started = Date.now();
    const tick = setInterval(() => {
      setElapsed(Date.now() - started);
    }, 50);
    return () => clearInterval(tick);
  }, []);

  // En modo mock no hay red que esperar: el store ya trae datos.
  if (isMockMode()) return { booting: false };

  const timedOut = elapsed >= maxMs;
  const done = ready && elapsed >= minMs;

  return { booting: !timedOut && !done };
}