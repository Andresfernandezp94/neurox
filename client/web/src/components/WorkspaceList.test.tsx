import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import { WorkspaceList } from "./WorkspaceList";
import type { Workspace } from "../api/workspaces";

function ws(over: Partial<Workspace> = {}): Workspace {
  return {
    id: "sixbell",
    name: "Sixbell",
    description: "product team",
    root: "/home/andres_fernandez/Sixbell",
    status: "active",
    icon: "IconIntegrations",
    is_default: false,
    sandbox: {
      enabled: true,
      writable_paths: [],
      readable_paths: ["${workspace}"],
      max_recursion_depth: 10,
    },
    created_at: "",
    updated_at: "",
    ...over,
  };
}

/** Escribe en el buscador. `fireEvent.change` y no `userEvent`: el repo no
 *  tiene `@testing-library/user-event` y usa `fireEvent` en sus tests. */
function buscar(q: string) {
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: q } });
}

/**
 * El `SearchBar` se calculaba y se pasaba, pero `workspaces.map` renderizaba
 * el array entero: escribir en el buscador no filtraba nada. Este test es el
 * que agarra esa regresion.
 */
describe("WorkspaceList: filtro", () => {
  const lista = [
    ws(),
    ws({ id: "projects", name: "Projects", root: "/home/andres_fernandez/projects" }),
    ws({ id: "notas", name: "Notas", description: "cosas del casa", root: "/tmp/notas" }),
  ];

  it("muestra todas cuando no hay busqueda", () => {
    render(<WorkspaceList workspaces={lista} />);
    expect(screen.getByText("Sixbell")).toBeInTheDocument();
    expect(screen.getByText("Projects")).toBeInTheDocument();
    expect(screen.getByText("Notas")).toBeInTheDocument();
  });

  it("filtra por nombre", async () => {
    render(<WorkspaceList workspaces={lista} />);
    buscar("proj");
    expect(screen.getByText("Projects")).toBeInTheDocument();
    expect(screen.queryByText("Sixbell")).toBeNull();
    expect(screen.queryByText("Notas")).toBeNull();
  });

  it("filtra por root", async () => {
    // El root es lo que uno tiene a mano cuando busca.
    render(<WorkspaceList workspaces={lista} />);
    buscar("/tmp/");
    expect(screen.getByText("Notas")).toBeInTheDocument();
    expect(screen.queryByText("Sixbell")).toBeNull();
  });

  it("filtra por descripcion", async () => {
    render(<WorkspaceList workspaces={lista} />);
    buscar("casa");
    expect(screen.getByText("Notas")).toBeInTheDocument();
    expect(screen.queryByText("Projects")).toBeNull();
  });

  it("avisa cuando no hay coincidencias", async () => {
    render(<WorkspaceList workspaces={lista} />);
    buscar("zzz");
    expect(screen.getByText("No matches.")).toBeInTheDocument();
  });

  it("no confunde 'sin resultados' con 'no hay workspaces'", async () => {
    const { rerender } = render(<WorkspaceList workspaces={lista} />);
    buscar("zzz");
    expect(screen.getByText("No matches.")).toBeInTheDocument();
    // El empty state de "no hay workspaces" es otro mensaje.
    expect(screen.queryByText("No workspaces configured yet.")).toBeNull();
    rerender(<WorkspaceList workspaces={[]} />);
    expect(screen.getByText("No workspaces configured yet.")).toBeInTheDocument();
  });
});

/**
 * Antes la card entera era un <button> y el IconButton de borrar quedaba
 * DENTRO (HTML invalido) con un `stopPropagation` para disimular: el click
 * se perdia y ademas el boton no era alcanzable por teclado como control
 * separado. El boton tiene que ser hermano, no hijo.
 */
describe("WorkspaceList: la card no anida botones", () => {
  it("el boton de borrar es hermano del area clickeable, no un hijo", () => {
    const { container } = render(
      <WorkspaceList workspaces={[ws()]} onOpenConfig={() => {}} onDelete={() => {}} />,
    );
    const card = container.querySelector(".workspace-card")!;
    const open = card.querySelector(".workspace-card__button")!;
    const del = card.querySelector(".workspace-card__delete")!;

    expect(open.tagName).toBe("BUTTON");
    expect(del.querySelector("button")).not.toBeNull();
    // Ningun boton dentro del boton.
    expect(open.querySelector("button")).toBeNull();
    expect(del.contains(open)).toBe(false);
  });

  it("no deja el boton de borrar pegado al borde del area clickeable", () => {
    // La regla que lo posiciona (`.workspace-card__delete`, absolute) tiene
    // que existir en el CSS; sin ella el borrar se caia del flujo.
    const card = ws();
    render(<WorkspaceList workspaces={[card]} onDelete={() => {}} />);
    expect(screen.getByTestId("workspace-delete-sixbell")).toBeInTheDocument();
  });
});

describe("WorkspaceList: acciones", () => {
  it("el boton de agregar solo aparece si hay onAdd", () => {
    const { rerender } = render(<WorkspaceList workspaces={[]} />);
    expect(screen.queryByTestId("workspace-add")).toBeNull();
    rerender(<WorkspaceList workspaces={[]} onAdd={() => {}} />);
    expect(screen.getByTestId("workspace-add")).toBeInTheDocument();
  });

  it("onAdd se dispara con el click", async () => {
    const onAdd = vi.fn();
    render(<WorkspaceList workspaces={[]} onAdd={onAdd} />);
    fireEvent.click(screen.getByTestId("workspace-add"));
    expect(onAdd).toHaveBeenCalledOnce();
  });

  it("onDelete se dispara con el id del workspace", async () => {
    const onDelete = vi.fn();
    render(
      <WorkspaceList
        workspaces={[ws(), ws({ id: "projects", name: "Projects" })]}
        onDelete={onDelete}
      />,
    );
    fireEvent.click(screen.getByTestId("workspace-delete-projects"));
    expect(onDelete).toHaveBeenCalledWith("projects");
  });

  it("onOpenConfig se dispara con el id", async () => {
    const onOpenConfig = vi.fn();
    render(
      <WorkspaceList
        workspaces={[ws(), ws({ id: "projects", name: "Projects" })]}
        onOpenConfig={onOpenConfig}
      />,
    );
    fireEvent.click(screen.getByTestId("workspace-card-open-projects"));
    expect(onOpenConfig).toHaveBeenCalledWith("projects");
  });

  it("muestra el default y el estado", () => {
    render(
      <WorkspaceList
        workspaces={[ws({ is_default: true }), ws({ id: "p2", name: "P2", status: "draft" })]}
      />,
    );
    const card = screen.getByTestId("workspace-card-sixbell");
    expect(within(card).getByText("default")).toBeInTheDocument();
    const card2 = screen.getByTestId("workspace-card-p2");
    expect(within(card2).getByText("draft")).toBeInTheDocument();
  });

  it("muestra el root y el conteo de paths del sandbox", () => {
    render(
      <WorkspaceList
        workspaces={[
          ws({
            sandbox: {
              enabled: true,
              writable_paths: ["${workspace}/.sdd"],
              readable_paths: ["${workspace}"],
              max_recursion_depth: 5,
            },
          }),
        ]}
      />,
    );
    const card = screen.getByTestId("workspace-card-sixbell");
    expect(within(card).getByText("/home/andres_fernandez/Sixbell")).toBeInTheDocument();
    expect(within(card).getByText("1 readable")).toBeInTheDocument();
    expect(within(card).getByText("1 writable")).toBeInTheDocument();
    expect(within(card).getByText("depth 5")).toBeInTheDocument();
  });

  it("distingue un workspace sin paths de sandbox de uno que si tiene", () => {
    render(
      <WorkspaceList
        workspaces={[
          ws({ sandbox: { enabled: false, writable_paths: [], readable_paths: [], max_recursion_depth: 10 } }),
        ]}
      />,
    );
    const card = screen.getByTestId("workspace-card-sixbell");
    expect(within(card).getByText("no sandbox paths")).toBeInTheDocument();
  });

  it("muestra el error sin perder la lista", () => {
    render(<WorkspaceList workspaces={[ws()]} error="boom" />);
    expect(screen.getByTestId("workspace-error")).toHaveTextContent("boom");
    expect(screen.getByText("Sixbell")).toBeInTheDocument();
  });

  it("distingue 'cargando' de 'no hay workspaces'", () => {
    // El empty state era inalcanzable antes porque el padre siempre pasaba
    // un array con un elemento hardcodeado. Con `loading` hay que poder
    // distinguir "todavia no se" de "no hay".
    const { rerender } = render(<WorkspaceList workspaces={[]} loading />);
    expect(screen.getByText("Loading workspaces…")).toBeInTheDocument();
    expect(screen.queryByText("No workspaces configured yet.")).toBeNull();

    rerender(<WorkspaceList workspaces={[]} loading={false} />);
    expect(screen.queryByText("Loading workspaces…")).toBeNull();
    expect(screen.getByText("No workspaces configured yet.")).toBeInTheDocument();
  });
});