import { useCallback, useEffect, useState } from "react";
import { listTodos, completeTodo, type TodoItem } from "../../api/todos";

/**
 * Panel de progreso del agente: la lista de todos, entre el chat y el
 * footer de input.
 *
 * EP-2026-10-03: el agente usa `todo_add` / `todo_done` para mostrar su
 * plan, pero hasta ahora ese progreso solo se veía abriendo el timeline
 * de cada mensaje — y ni siquiera después de recargar, porque el daemon
 * no lo persistia (ver el fix de `hydrateMessages`).
 *
 * Decisiones:
 * - **Colapsado por default.** El chat es lo principal; el panel está
 *   para glancear "cuántas quedan", no para replaces la conversación.
 * - **Lee del daemon, no del timeline.** Un POST a `/v1/tools/todo_list`
 *   cada 4s mientras el agente streaméa, y 15s cuando está idle. Es la
 *   misma fuente de verdad que ve el agente, así que no puede divergir.
 * - **Poll, no WS.** `/v1/events` no emite un evento específico de todo;
 *   agregar uno sería plumbing extra para algo que un poll de 4s resuelve.
 * - **El checkbox marca hecho** vía `todo_done`, que acepta id completo o
 *   prefijo. Es la misma tool que usa el agente.
 */

interface TodoPanelProps {
  /** `true` mientras el agente está streameando: baja el poll a 4s. */
  isStreaming: boolean;
}

const POLL_ACTIVE_MS = 4_000;
const POLL_IDLE_MS = 15_000;

export function TodoPanel({ isStreaming }: TodoPanelProps) {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [open, setOpen] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    // `listTodos` ya falla suave (devuelve [] si el daemon no
    // responde), pero el poll llama a `refresh` con `void` y una
    // rechazo ahí se vuelve un unhandled rejection. El catch de acá es
    // la segunda red: si algo más falla, el panel se queda con la lista
    // anterior en vez de romper el chat.
    try {
      setTodos(await listTodos());
    } catch {
      // se conserva el estado previo
    }
  }, []);

  // Poll. El intervalo depende de si el agente está trabajando: durante
  // un stream los cambios son constantes y conviene refrescar seguido;
  // parado, 15s alcanza sin gastar requests.
  useEffect(() => {
    void refresh();
    const id = setInterval(
      () => void refresh(),
      isStreaming ? POLL_ACTIVE_MS : POLL_IDLE_MS,
    );
    return () => clearInterval(id);
  }, [refresh, isStreaming]);

  const handleToggle = useCallback(
    async (todo: TodoItem) => {
      // Optimista: el checkbox se mueve al instante, no al volver la
      // respuesta del daemon.
      const wasDone = todo.status === "done";
      setBusyId(todo.id);
      // Optimista: el checkbox se mueve ya y se revierte si el daemon
      // rechaza. Un F5 a mitad de request no debe dejar el panel colgado.
      setTodos((prev) =>
        prev.map((t) =>
          t.id === todo.id
            ? { ...t, status: wasDone ? "pending" : "done" }
            : t,
        ),
      );
      try {
        const ok = await completeTodo(todo.id);
        if (!ok) await refresh();
      } finally {
        setBusyId(null);
        void refresh();
      }
    },
    [refresh],
  );

  const pending = todos.filter((t) => t.status !== "done").length;
  const done = todos.length - pending;

  // Sin todos: no se renderiza nada. Un panel vacío es ruido.
  if (todos.length === 0) return null;

  return (
    <div
      className="todo-panel"
      data-testid="todo-panel"
      data-open={open ? "true" : "false"}
    >
      <button
        type="button"
        className="todo-panel__toggle"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        data-testid="todo-panel-toggle"
      >
        {/* El glyph rota con el estado: ▶ cerrado, ▼ abierto. Es el
            mismo idioma visual que usa el resto del chat para los
            nodos plegables del timeline. */}
        <span className="todo-panel__chevron" aria-hidden="true">
          {open ? "▼" : "▶"}
        </span>
        <span className="todo-panel__label">
          {pending > 0 ? `${pending} pendiente${pending === 1 ? "" : "s"}` : "Todo listo"}
        </span>
        {done > 0 && (
          <span className="todo-panel__count" data-testid="todo-panel-done">
            {done}/{todos.length}
          </span>
        )}
      </button>

      {open && (
        <ul className="todo-panel__list" data-testid="todo-panel-list">
          {todos.map((todo) => {
            const isDone = todo.status === "done";
            return (
              <li
                key={todo.id}
                className="todo-panel__item"
                data-testid={`todo-item-${todo.id}`}
                data-done={isDone ? "true" : "false"}
              >
                <label className="todo-panel__label-row">
                  <input
                    type="checkbox"
                    checked={isDone}
                    disabled={busyId === todo.id}
                    onChange={() => void handleToggle(todo)}
                    data-testid={`todo-check-${todo.id}`}
                    aria-label={
                      isDone
                        ? `Reabrir ${todo.content}`
                        : `Marcar ${todo.content} como hecho`
                    }
                  />
                  <span className="todo-panel__text">{todo.content}</span>
                </label>
                {todo.priority !== "normal" && (
                  <span
                    className={`todo-panel__priority todo-panel__priority--${todo.priority}`}
                  >
                    {todo.priority}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}