import { apiPost } from "./client";

/**
 * API del store de todos (`todo_*`).
 *
 * Las tools `todo_add` / `todo_done` / `todo_remove` / `todo_clear`
 * escriben en un JSON plano que el daemon expone por HTTP. No hace
 * falta pasar por el agente para leer el estado: el panel de la web
 * consulta `/v1/tools/todo_list` directo.
 *
 * El store es compartido entre sesiones (no tiene `session_id`), que es
 * exactamente como lo define `tools-engine/src/tools/task_management/`.
 */

export interface TodoItem {
  id: string;
  content: string;
  priority: "low" | "normal" | "high";
  status: "pending" | "done";
  created_at: number;
  done_at?: number | null;
}

export interface TodoListResponse {
  ok: boolean;
  tool: string;
  result: string;
  error?: string;
}

const EMPTY: TodoItem[] = [];

/**
 * Lista los todos pendientes y completados.
 *
 * Falla suave: si el daemon está caído devuelve `[]` en vez de tirar,
 * porque el panel es decorativo y no debe romper el chat.
 */
export async function listTodos(): Promise<TodoItem[]> {
  try {
    const res = await apiPost<TodoListResponse>("/v1/tools/todo_list/invoke", {
      args: { status: "all" },
    });
    if (!res.ok) return EMPTY;
    return parseTodoList(res.result);
  } catch {
    return EMPTY;
  }
}

/** Marca un todo como hecho. Acepta id completo o prefijo. */
export async function completeTodo(id: string): Promise<boolean> {
  try {
    const res = await apiPost<TodoListResponse>("/v1/tools/todo_done/invoke", {
      args: { id },
    });
    return res.ok === true;
  } catch {
    return false;
  }
}

/**
 * `todo_list` devuelve texto plano, no JSON: cada línea es
 * `[ ] #<id> <contenido> (<prioridad>)`.
 *
 * Se parsea así y no con `JSON.parse` porque ese es el contrato que
 * expone la tool — si algún día pasa a devolver JSON, el fallback de
 * abajo lo cubre sin tocar el panel.
 */
export function parseTodoList(raw: string): TodoItem[] {
  const trimmed = raw.trim();
  if (!trimmed || trimmed === "no todos") return EMPTY;

  // Camino JSON: por si la tool cambia de contrato.
  if (trimmed.startsWith("[")) {
    try {
      const parsed: unknown = JSON.parse(trimmed);
      if (Array.isArray(parsed)) return parsed.filter(isTodo);
    } catch {
      /* cae al parser de texto */
    }
  }

  const out: TodoItem[] = [];
  for (const line of trimmed.split("\n")) {
    // `[x] #t99f2d Probar shell: crear directorio (high)`
    const m = line.match(/^\[( |x|X)\]\s*#(\S+)\s*(.*?)\s*(?:\((low|normal|high)\))?$/);
    if (!m) continue;
    const mark = m[1] ?? " ";
    const id = m[2] ?? "";
    const content = m[3] ?? "";
    const priority = m[4];
    out.push({
      id,
      content: content.trim(),
      priority: (priority as TodoItem["priority"]) ?? "normal",
      status: mark === " " ? "pending" : "done",
      created_at: 0,
    });
  }
  return out;
}

function isTodo(v: unknown): v is TodoItem {
  return (
    typeof v === "object" &&
    v !== null &&
    typeof (v as TodoItem).id === "string" &&
    typeof (v as TodoItem).content === "string"
  );
}

