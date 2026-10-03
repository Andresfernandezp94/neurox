import type { TimelineEntry } from "../../../types";

/**
 * Reconstruye el `timeline` de un mensaje assistant a partir de las
 * filas planas que devuelve `GET /v1/sessions/:id/messages`.
 *
 * EP-2026-10-03: el daemon persiste cada evento del stream por
 * separado (`thinking`, `tool_call`, `tool`, y al final el `assistant`
 * con el texto). Antes solo se guardaban user+assistant, asi que un F5
 * durante un stream dejaba la sesion sin progreso visible.
 *
 * El agrupador existe porque el resto de la app (TimelineRenderer,
 * ChatSearchBar, los tests) trabaja con `Message.timeline` y no con
 * filas sueltas. Acá se junta cada racha de thinking/tool_call/tool
 * con el mensaje assistant que las cierra, respetando el orden en que
 * el agente las emitio.
 *
 * Reglas:
 * - `thinking` + `tool_call` + `tool` que aparecen después de un user y
 *   antes del siguiente assistant se acumulan en un timeline.
 * - Los pares `tool_call` → `tool` se funden en UNA entrada `{type:'tool'}`
 *   con `args` + `result`, que es lo que `TimelineRenderer` espera.
 * - Un `tool_call` sin `tool` todavía (tool en vuelo) se emite igual,
 *   con `result` undefined: el renderer lo muestra como running.
 * - Fragmentos de `thinking` contiguos se concatenan con `\n\n`, que es
 *   como el backend los separa.
 * - Un `assistant` sin timeline previo conserva su `thinking` de columna y
 *   no inventa timeline: es un turno corto que no usó tools.
 */

/** Fila cruda de `GET /v1/sessions/:id/messages`. */
export interface StoredMessage {
  id: number;
  role: string;
  content: string;
  thinking?: string | null;
  ts: string;
  tool_name?: string | null;
  tool_call_id?: string | null;
}

/** Fila ya convertida a `Message`, con el timeline resuelto. */
export type HydratedMessage = {
  id: number;
  role: 'user' | 'assistant' | 'system' | 'tool' | 'thinking' | 'tool_call';
  content: string;
  ts: string;
  thinking?: string;
  tool_name?: string | null;
  timeline?: TimelineEntry[];
};

const isProgress = (role: string) =>
  role === 'thinking' || role === 'tool_call' || role === 'tool';

/**
 * Fusiona las filas de una sesión en mensajes con timeline.
 *
 * `sessionId` se usa solo para cumplir el shape de `Message`.
 */
export function hydrateMessages(
  rows: StoredMessage[],
  sessionId: string,
): HydratedMessage[] {
  const out: HydratedMessage[] = [];

  // Filas de progreso que todavia no tienen un assistant que las cierre.
  let pending: TimelineEntry[] = [];
  let openTool: TimelineEntry | null = null;

  const takePending = (): TimelineEntry[] => {
    const taken = pending;
    pending = [];
    openTool = null;
    return taken;
  };

  for (const row of rows) {
    if (!isProgress(row.role)) {
      const base: HydratedMessage = {
        id: row.id,
        role: row.role as HydratedMessage['role'],
        content: row.content,
        ts: row.ts,
      };
      if (row.role === 'assistant' && row.thinking) {
        base.thinking = row.thinking;
      }
      const collected = takePending();
      if (collected.length > 0) {
        // El progreso pertenece al mensaje que viene DESPUES: el
        // assistant cierra la racha de tool/thinking que lo precede.
        // Un `user` en el medio significa que la racha anterior quedo
        // huerfana (el stream murio antes del texto final), asi que se
        // emite como assistant vacio en vez de pegarse al mensaje nuevo.
        if (row.role === 'assistant') {
          base.timeline = collected;
        } else {
          out.push({
            id: -collected.length,
            role: 'assistant',
            content: '',
            ts: new Date().toISOString(),
            timeline: collected,
          });
        }
      }
      out.push(base);
      continue;
    }

    if (row.role === 'thinking') {
      const last = pending[pending.length - 1];
      if (last && last.type === 'thinking') {
        last.text = `${last.text}\n\n${row.content}`;
      } else {
        pending.push({ type: 'thinking', text: row.content });
      }
      continue;
    }

    if (row.role === 'tool_call') {
      // Un tool_call siempre cierra el anterior: si el modelo paraleliza,
      // los results llegan en otro orden y se emparejan por posicion
      // (el daemon no persiste call_id aun, ver EP-2026-10-03).
      const entry: TimelineEntry = {
        type: 'tool',
        tool: row.tool_name ?? 'unknown',
        args: safeParse(row.content),
        iteration: pending.filter((e) => e.type === 'tool').length,
      };
      pending.push(entry);
      openTool = entry;
      continue;
    }

    // role === 'tool': es el resultado. Si hay un tool_call abierto, lo
    // completa; si no, se emite suelto (el daemon persistio el result
    // de una tool cuyo call se perdio).
    if (openTool && openTool.type === 'tool') {
      openTool.result = row.content;
      openTool = null;
    } else {
      pending.push({
        type: 'tool',
        tool: row.tool_name ?? 'unknown',
        result: row.content,
        iteration: pending.filter((e) => e.type === 'tool').length,
      });
    }
  }

  const rest = takePending();
  if (rest.length > 0) {
    out.push({
      id: -rest.length,
      role: 'assistant',
      content: '',
      ts: new Date().toISOString(),
      timeline: rest,
    });
  }
  void sessionId;
  return out;
}

/** El contenido de un tool_call es el args JSON. Si no parsea, se deja
 * como string: el renderer tiene un fallback para texto plano. */
function safeParse(raw: string): unknown {
  const trimmed = raw.trim();
  if (!trimmed) return undefined;
  try {
    return JSON.parse(trimmed);
  } catch {
    return raw;
  }
}