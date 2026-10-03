import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { TodoPanel } from "./TodoPanel";
import { listTodos, completeTodo, type TodoItem } from "../../api/todos";

vi.mock("../../api/todos", () => ({
  listTodos: vi.fn(),
  completeTodo: vi.fn(),
}));

const mkTodo = (over: Partial<TodoItem> = {}): TodoItem => ({
  id: "t1",
  content: "Probar shell",
  priority: "normal",
  status: "pending",
  created_at: 0,
  ...over,
});

describe("TodoPanel", () => {
  beforeEach(() => {
    vi.mocked(listTodos).mockReset();
    vi.mocked(completeTodo).mockReset();
    vi.mocked(listTodos).mockResolvedValue([]);
    vi.mocked(completeTodo).mockResolvedValue(true);
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("no renderiza nada cuando no hay todos", async () => {
    const { container } = render(<TodoPanel isStreaming={false} />);
    await waitFor(() => expect(listTodos).toHaveBeenCalled());
    expect(container.querySelector('[data-testid="todo-panel"]')).toBeNull();
  });

  it("muestra el panel con el conteo de pendientes, colapsado", async () => {
    vi.mocked(listTodos).mockResolvedValue([
      mkTodo({ id: "a" }),
      mkTodo({ id: "b" }),
      mkTodo({ id: "c", status: "done" }),
    ]);
    render(<TodoPanel isStreaming={false} />);

    await waitFor(() => expect(screen.getByTestId("todo-panel")).toBeTruthy());
    expect(screen.getByText("2 pendientes")).toBeTruthy();
    expect(screen.getByTestId("todo-panel-done").textContent).toBe("1/3");
    // Colapsado: la lista no está en el DOM.
    expect(screen.queryByTestId("todo-panel-list")).toBeNull();
  });

  it("abre y cierra la lista al clickear el toggle", async () => {
    vi.mocked(listTodos).mockResolvedValue([mkTodo({ id: "a" })]);
    render(<TodoPanel isStreaming={false} />);

    const toggle = await screen.findByTestId("todo-panel-toggle");
    expect(screen.queryByTestId("todo-panel-list")).toBeNull();

    fireEvent.click(toggle);
    expect(screen.getByTestId("todo-panel-list")).toBeTruthy();
    expect(screen.getByText("Probar shell")).toBeTruthy();

    fireEvent.click(toggle);
    expect(screen.queryByTestId("todo-panel-list")).toBeNull();
  });

  it("usa el singular para un solo pendiente", async () => {
    vi.mocked(listTodos).mockResolvedValue([mkTodo({ id: "a" })]);
    render(<TodoPanel isStreaming={false} />);
    await waitFor(() => expect(screen.getByText("1 pendiente")).toBeTruthy());
  });

  it("anuncia 'Todo listo' cuando no queda nada pendiente", async () => {
    vi.mocked(listTodos).mockResolvedValue([
      mkTodo({ id: "a", status: "done" }),
    ]);
    render(<TodoPanel isStreaming={false} />);
    await waitFor(() => expect(screen.getByText("Todo listo")).toBeTruthy());
  });

  it("marca hecho vía completeTodo y refresca", async () => {
    vi.mocked(listTodos).mockResolvedValue([mkTodo({ id: "abc123" })]);
    render(<TodoPanel isStreaming={false} />);

    // El panel arranca colapsado: primero se abre.
    fireEvent.click(await screen.findByTestId("todo-panel-toggle"));
    const check = (await screen.findByTestId(
      "todo-check-abc123",
    )) as HTMLInputElement;
    expect(check.checked).toBe(false);

    fireEvent.click(check);
    await waitFor(() => expect(completeTodo).toHaveBeenCalledWith("abc123"));
    // El refresh post-mutacion suma al poll inicial: al menos 2.
    await waitFor(() => expect(vi.mocked(listTodos).mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("revierte el estado optimista si el daemon rechaza", async () => {
    vi.mocked(listTodos).mockResolvedValue([mkTodo({ id: "abc123" })]);
    vi.mocked(completeTodo).mockResolvedValue(false);
    render(<TodoPanel isStreaming={false} />);

    fireEvent.click(await screen.findByTestId("todo-panel-toggle"));
    const check = (await screen.findByTestId(
      "todo-check-abc123",
    )) as HTMLInputElement;
    fireEvent.click(check);

    // El rechazo dispara un refresh que vuelve a traer el pendiente.
    await waitFor(() =>
      expect(vi.mocked(listTodos).mock.calls.length).toBeGreaterThanOrEqual(2),
    );
    const after = screen.getByTestId("todo-check-abc123") as HTMLInputElement;
    expect(after.checked).toBe(false);
  });

  it("no rompe si listTodos falla (daemon caído)", async () => {
    vi.mocked(listTodos).mockRejectedValueOnce(new Error("boom"));
    const { container } = render(<TodoPanel isStreaming={false} />);
    await waitFor(() => expect(listTodos).toHaveBeenCalled());
    expect(container.querySelector('[data-testid="todo-panel"]')).toBeNull();
  });

  it("muestra la prioridad solo cuando no es normal", async () => {
    vi.mocked(listTodos).mockResolvedValue([
      mkTodo({ id: "a", priority: "high" }),
      mkTodo({ id: "b", priority: "normal" }),
    ]);
    render(<TodoPanel isStreaming={false} />);
    fireEvent.click(await screen.findByTestId("todo-panel-toggle"));
    await waitFor(() =>
      expect(screen.getByTestId("todo-panel-list")).toBeTruthy(),
    );
    // Solo el `high` lleva badge: `normal` no se etiqueta.
    expect(
      document.querySelectorAll(".todo-panel__priority"),
    ).toHaveLength(1);
    expect(screen.getByText("high")).toBeTruthy();
  });

  it("poll más seguido mientras el agente streamea", async () => {
    vi.mocked(listTodos).mockResolvedValue([mkTodo()]);
    render(<TodoPanel isStreaming />);
    await waitFor(() => expect(listTodos).toHaveBeenCalledTimes(1));

    await vi.advanceTimersByTimeAsync(4_500);
    expect(vi.mocked(listTodos).mock.calls.length).toBeGreaterThan(1);
  });

  it("poll lento cuando está idle", async () => {
    vi.mocked(listTodos).mockResolvedValue([mkTodo()]);
    render(<TodoPanel isStreaming={false} />);
    await waitFor(() => expect(listTodos).toHaveBeenCalledTimes(1));

    // 4.5s no alcanza para el intervalo idle de 15s.
    await vi.advanceTimersByTimeAsync(4_500);
    expect(listTodos).toHaveBeenCalledTimes(1);
  });
});