import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { SandboxTab } from "./SandboxTab";

vi.mock("../api/sandbox", () => ({
  putSandbox: vi.fn(),
}));

vi.mock("../api/default", () => ({
  getDefaultAgentStatus: vi.fn(),
}));

import { putSandbox } from "../api/sandbox";
import { getDefaultAgentStatus } from "../api/default";

const mockPut = putSandbox as ReturnType<typeof vi.fn>;
const mockStatus = getDefaultAgentStatus as ReturnType<typeof vi.fn>;

const base = {
  enabled: true,
  writable_paths: [] as string[],
  readable_paths: [] as string[],
  max_recursion_depth: 10,
};

describe("SandboxTab: badge de las secciones", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockStatus.mockResolvedValue(null);
  });

  /**
   * Este es el sitio del bug: `CollapsibleSection` ya envuelve el badge en
   * un `<span class="collapsible__badge">`, y el panel pasaba otro
   * `<span class="collapsible__badge">` como contenido. Quedaba un badge
   * dentro del badge (doble borde, doble relleno, padding al doble) y la
   * variante accent quedaba enmarcada por un pill neutral.
   *
   * Se cuenta el numero de elementos con la clase: el que trae el wrapper
   * del componente y el que el panel ya no debe pasar.
   */
  it("no anida badges en las secciones de paths", async () => {
    mockPut.mockResolvedValue({
      ...base,
      writable_paths: ["${workspace}/.sdd"],
      readable_paths: ["${home}"],
    });

    const { container } = render(<SandboxTab />);

    await screen.findByText("Writable paths");
    await waitFor(() =>
      expect(container.querySelectorAll(".collapsible__badge").length).toBeGreaterThan(0),
    );

    // Ningun badge contiene otro badge.
    const badges = container.querySelectorAll(".collapsible__badge");
    for (const b of badges) {
      expect(b.querySelectorAll(".collapsible__badge")).toHaveLength(0);
      expect(b.querySelector(".collapsible__badge")).toBeNull();
    }
  });

  it("el badge muestra el conteo y marca la variante accent si hay paths", async () => {
    mockPut.mockResolvedValue({
      ...base,
      writable_paths: ["${workspace}/.sdd", "${workspace}/notas"],
      readable_paths: [],
    });

    render(<SandboxTab />);

    const writable = (await screen.findByText("Writable paths")).closest(
      ".collapsible",
    )!;
    const badge = writable.querySelector(".collapsible__badge")!;
    expect(badge.textContent).toBe("2");
    expect(badge).toHaveClass("collapsible__badge--accent");

    const readable = screen.getByText("Readable paths").closest(".collapsible")!;
    const badgeVacio = readable.querySelector(".collapsible__badge")!;
    expect(badgeVacio.textContent).toBe("0");
    // Sin paths no hay variante accent: el cero es el estado informative.
    expect(badgeVacio).not.toHaveClass("collapsible__badge--accent");
  });
});
