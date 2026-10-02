import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import type { ReactNode } from "react";
import { NotificationStack } from "./NotificationStack";
import { NotificationsProvider, useNotifications } from "../../../store/NotificationsContext";

/** Boton que publica notificaciones, para no depender del orden de render. */
function Controls() {
  const notify = useNotifications();
  return (
    <div>
      <button
        type="button"
        onClick={() => notify.error("primer error")}
        data-testid="push-first"
      >
        primero
      </button>
      <button
        type="button"
        onClick={() => notify.error("segundo error")}
        data-testid="push-second"
      >
        segundo
      </button>
    </div>
  );
}

function setup(children?: ReactNode) {
  return render(
    <NotificationsProvider>
      <Controls />
      {children ?? <NotificationStack />}
    </NotificationsProvider>,
  );
}

describe("NotificationStack", () => {
  it("no renderiza nada sin notificaciones", () => {
    setup();
    expect(screen.queryByTestId("notification-stack")).not.toBeInTheDocument();
  });

  it("con dos notificaciones se apilan: una debajo de la otra", () => {
    // El orden en el DOM es el de arriba hacia abajo, y el contenedor es
    // `flex-direction: column`, asi que la segunda queda DEBAJO de la
    // primera. El orden de llegada es el de lectura: la mas reciente al
    // final, junto al input.
    setup();
    fireEvent.click(screen.getByTestId("push-first"));
    fireEvent.click(screen.getByTestId("push-second"));

    const stack = screen.getByTestId("notification-stack");
    const rows = stack.querySelectorAll(".notification");
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain("primer error");
    expect(rows[1]!.textContent).toContain("segundo error");
  });

  it("el error usa la clase de error y role=alert", () => {
    setup();
    fireEvent.click(screen.getByTestId("push-first"));
    const row = screen.getByRole("alert");
    expect(row).toHaveClass("notification--error");
  });

  it("la X cierra solo esa notificacion y deja las otras", () => {
    setup();
    fireEvent.click(screen.getByTestId("push-first"));
    fireEvent.click(screen.getByTestId("push-second"));

    const closeFirst = screen.getByRole("button", { name: /primer error/ });
    fireEvent.click(closeFirst);

    expect(screen.queryByText("primer error")).not.toBeInTheDocument();
    expect(screen.getByText("segundo error")).toBeInTheDocument();
  });

  it("la X describe CUAL aviso cierra", () => {
    // Con varias, un "Cerrar" repetido no le dice al lector de pantalla
    // que boton es cual.
    setup();
    fireEvent.click(screen.getByTestId("push-first"));
    fireEvent.click(screen.getByTestId("push-second"));
    expect(
      screen.getByRole("button", { name: "Cerrar aviso: primer error" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Cerrar aviso: segundo error" }),
    ).toBeInTheDocument();
  });

  it("avisos iguales se apilan igual: tres filas, no una con contador", () => {
    function Repeater() {
      const notify = useNotifications();
      return (
        <button
          type="button"
          data-testid="push-twice"
          onClick={() => {
            notify.error("repetido");
            notify.error("repetido");
            notify.error("repetido");
          }}
        >
          x3
        </button>
      );
    }
    const { container } = render(
      <NotificationsProvider>
        <Repeater />
        <NotificationStack />
      </NotificationsProvider>,
    );

    fireEvent.click(screen.getByTestId("push-twice"));
    // Sin agrupar: tres filas independientes, una debajo de otra.
    expect(container.querySelectorAll(".notification")).toHaveLength(3);
    expect(screen.getAllByText("repetido")).toHaveLength(3);
  });

  it("cada aviso atras es mas chico que el de al frente", () => {
    // El mazo se achica por profundidad. Con el mismo ancho, los de abajo
    // parecian duplicados del de arriba en vez de cards de un mazo.
    //
    // Se asserta el custom property `--depth` que CSS usa para escalar, y
    // no el `transform` computado: en jsdom no hay layout, asi que el
    // `scale()` no se resuelve y testearlo daria falso verde.
    function Push3() {
      const notify = useNotifications();
      return (
        <button
          type="button"
          data-testid="push3"
          onClick={() => {
            notify.error("a");
            notify.alert("b");
            notify.error("c");
          }}
        >
          x3
        </button>
      );
    }
    const { container } = render(
      <NotificationsProvider>
        <Push3 />
        <NotificationStack />
      </NotificationsProvider>,
    );
    fireEvent.click(screen.getByTestId("push3"));

    const rows = Array.from(container.querySelectorAll<HTMLElement>(".notification"));
    expect(rows).toHaveLength(3);
    // DOM en orden de llegada, asi que el ultimo es el de al frente.
    const depth = rows.map((r) => r.style.getPropertyValue("--depth").trim());
    expect(depth).toEqual(["2", "1", "0"]);
  });

  it("con 8 avisos la profundidad visual se acota pero ninguno se pierde", () => {
    // El tope acota el escalonado, no la cantidad. Los 8 siguen en el DOM
    // (se cierran de a uno) y los mas viejos se acumulan en la maxima
    // profundidad, que es lo que evita que el mazo crezca sin limite.
    function Push8() {
      const notify = useNotifications();
      return (
        <button
          type="button"
          data-testid="push8"
          onClick={() => {
            for (let i = 0; i < 8; i += 1) notify.error(`e${i}`);
          }}
        >
          x8
        </button>
      );
    }
    const { container } = render(
      <NotificationsProvider>
        <Push8 />
        <NotificationStack />
      </NotificationsProvider>,
    );
    fireEvent.click(screen.getByTestId("push8"));

    const rows = Array.from(container.querySelectorAll<HTMLElement>(".notification"));
    expect(rows).toHaveLength(8);
    const depth = rows.map((r) => Number(r.style.getPropertyValue("--depth")));
    // DOM de atras hacia adelante, con el piso en MAX_VISUAL_DEPTH.
    expect(depth).toEqual([4, 4, 4, 4, 3, 2, 1, 0]);
    // Solo el de al frente muestra su contenido; los demas, solo la franja.
    expect(container.querySelectorAll(".notification--front")).toHaveLength(1);
    expect(container.querySelectorAll(".notification--behind")).toHaveLength(7);
  });

  it("cerrar el de al frente promueve el siguiente: se van de a uno", () => {
    function Push2() {
      const notify = useNotifications();
      return (
        <button
          type="button"
          data-testid="push2"
          onClick={() => {
            notify.error("viejo");
            notify.error("nuevo");
          }}
        >
          x2
        </button>
      );
    }
    const { container } = render(
      <NotificationsProvider>
        <Push2 />
        <NotificationStack />
      </NotificationsProvider>,
    );
    fireEvent.click(screen.getByTestId("push2"));

    // Al frente esta "nuevo". Cerrandolo, "viejo" pasa a ser el de al
    // frente y queda cerrable: es lo que hace que en un mazo todos se
    // puedan cerrar sin tener que destaparlos a mano.
    fireEvent.click(screen.getByRole("button", { name: "Cerrar aviso: nuevo" }));
    expect(screen.queryByText("nuevo")).not.toBeInTheDocument();
    expect(container.querySelectorAll(".notification--front")).toHaveLength(1);
    expect(screen.getByText("viejo")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Cerrar aviso: viejo" }),
    ).toBeInTheDocument();
  });

  it("un proceso usa role=status y no interrumpe al lector", () => {
    // Un spinner no debe cortar lo que el usuario esta leyendo, asi que
    // va como `status` y no como `alert`.
    function StartProcess() {
      const notify = useNotifications();
      return (
        <button type="button" data-testid="start" onClick={() => notify.startProcess("guardando")}>
          start
        </button>
      );
    }
    render(
      <NotificationsProvider>
        <StartProcess />
        <NotificationStack />
      </NotificationsProvider>,
    );
    fireEvent.click(screen.getByTestId("start"));
    const row = screen.getByRole("status");
    expect(row).toHaveClass("notification--process");
  });
});
