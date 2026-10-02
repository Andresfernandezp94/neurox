import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import {
  NotificationsProvider,
  MAX_VISIBLE,
  useNotificationItems,
  useNotifications,
} from "./NotificationsContext";

function wrapper({ children }: { children: ReactNode }) {
  return <NotificationsProvider>{children}</NotificationsProvider>;
}

/** Api + los items vivos, con los derivados que los tests assertan. */
function setup() {
  return renderHook(
    () => {
      const api = useNotifications();
      const items = useNotificationItems();
      return {
        api,
        items,
        itemsLength: items.length,
        messages: items.map((i) => i.message),
        kinds: items.map((i) => i.kind),
      };
    },
    { wrapper },
  );
}

describe("NotificationsContext", () => {
  it("agrega un error y lo devuelve con su id", () => {
    const { result } = setup();
    let id = 0;
    act(() => {
      id = result.current.api.error("se rompio");
    });
    expect(id).toBeGreaterThan(0);
  });

  it("un mensaje vacio no publica nada y devuelve id 0", () => {
    // Id 0 significa "no se publico". Un id valido sobre un mensaje vacio
    // invitaba al call site a resolver o fallar algo inexistente.
    const { result } = setup();
    let id = 99;
    act(() => {
      id = result.current.api.error("   ");
    });
    expect(id).toBe(0);
  });

  it("NO agrupa avisos iguales: cada uno es su propia fila", () => {
    // Agruparlos con un contador ("x3") se leia como un resumen. Lo que
    // se quiere es verlos uno debajo del otro, que es como se entienden
    // los que importan: tres tool calls que fallaron con el mismo error
    // son tres cosas que pasó, no una.
    const { result } = setup();
    act(() => {
      result.current.api.error("mismo");
      result.current.api.error("mismo");
      result.current.api.error("mismo");
    });
    expect(result.current.itemsLength).toBe(3);
    expect(result.current.messages).toEqual(["mismo", "mismo", "mismo"]);
  });

  it("descarta las mas viejas pasado el tope", () => {
    const { result } = setup();
    act(() => {
      for (let i = 0; i < MAX_VISIBLE + 3; i += 1) {
        result.current.api.error(`e${i}`);
      }
    });
    expect(result.current.itemsLength).toBe(MAX_VISIBLE);
    // La mas nueva esta; la primera se cayo.
    expect(result.current.messages).not.toContain("e0");
    expect(result.current.messages).toContain(`e${MAX_VISIBLE + 2}`);
  });

  it("dismiss saca solo la del id", () => {
    const { result } = setup();
    let first = 0;
    let second = 0;
    act(() => {
      first = result.current.api.error("primero");
      second = result.current.api.error("segundo");
    });
    act(() => {
      result.current.api.dismiss(first);
    });
    expect(result.current.messages).toEqual(["segundo"]);
    expect(second).not.toBe(first);
  });

  it("proceso: desaparece al resolverse", () => {
    const { result } = setup();
    let id = 0;
    act(() => {
      id = result.current.api.startProcess("guardando");
    });
    expect(result.current.messages).toEqual(["guardando"]);
    act(() => {
      result.current.api.resolveProcess(id);
    });
    expect(result.current.itemsLength).toBe(0);
  });

  it("proceso: al fallar pasa a error y QUEDA esperando la X", () => {
    // Lo pedido: un proceso solo se queda si falla.
    const { result } = setup();
    let id = 0;
    act(() => {
      id = result.current.api.startProcess("guardando");
    });
    act(() => {
      result.current.api.failProcess(id, "no se pudo guardar");
    });
    expect(result.current.itemsLength).toBe(1);
    expect(result.current.kinds).toEqual(["error"]);
    expect(result.current.messages).toEqual(["no se pudo guardar"]);
  });

  it("proceso: fallar sin detalle lo saca en vez de dejarlo girando", () => {
    const { result } = setup();
    let id = 0;
    act(() => {
      id = result.current.api.startProcess("guardando");
    });
    act(() => {
      result.current.api.failProcess(id, "");
    });
    expect(result.current.itemsLength).toBe(0);
  });

  it("runProcess resuelve y devuelve el valor", async () => {
    const { result } = setup();
    let out: number | null = null;
    await act(async () => {
      out = await result.current.api.runProcess("calculando", async () => 42);
    });
    expect(out).toBe(42);
    expect(result.current.itemsLength).toBe(0);
  });

  it("runProcess falla: deja el error y vuelve a lanzar", async () => {
    // Vuelve a lanzar ademas de notificar: el call site casi siempre
    // tambien tiene que limpiar su estado local.
    const { result } = setup();
    let caught: unknown = null;
    await act(async () => {
      try {
        await result.current.api.runProcess("calculando", async () => {
          throw new Error("boom");
        });
      } catch (e) {
        caught = e;
      }
    });
    expect((caught as Error).message).toBe("boom");
    expect(result.current.kinds).toEqual(["error"]);
    expect(result.current.messages).toEqual(["boom"]);
  });

  it("clear vacia todo", () => {
    const { result } = setup();
    act(() => {
      result.current.api.error("a");
      result.current.api.alert("b");
    });
    act(() => {
      result.current.api.clear();
    });
    expect(result.current.itemsLength).toBe(0);
  });
});
