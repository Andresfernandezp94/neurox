import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CollapsibleSection } from "./CollapsibleSection";

describe("CollapsibleSection", () => {
  it("renders title and children", () => {
    render(
      <CollapsibleSection title="General">
        <p>cuerpo</p>
      </CollapsibleSection>,
    );
    expect(screen.getByText("General")).toBeInTheDocument();
    expect(screen.getByText("cuerpo")).toBeInTheDocument();
  });

  it("no renders the badge when it was not passed", () => {
    const { container } = render(
      <CollapsibleSection title="General">x</CollapsibleSection>,
    );
    expect(container.querySelector(".collapsible__badge")).toBeNull();
  });

  /**
   * El wrapper del badge ya lleva `collapsible__badge`. Si el caller pasa
   * un `<span className="collapsible__badge">` como contenido, queda un
   * badge dentro del badge: dos bordes, dos rellenos y el padding
   * horizontal duplicado. Era lo que pasaba en SandboxTab, que ademas
   * envolvia el badge para elegir la variante accent, con lo que la
   * variante quedaba enmarcada por un pill neutral.
   *
   * El contrato es: el contenido del badge va plano y la variante se pide
   * con `badgeClassName`.
   */
  it("wraps the badge content in a single badge element", () => {
    const { container } = render(
      <CollapsibleSection title="Readable paths" badge={3}>
        x
      </CollapsibleSection>,
    );
    const badges = container.querySelectorAll(".collapsible__badge");
    expect(badges).toHaveLength(1);
    expect(badges[0]!.textContent).toBe("3");
  });

  it("applies badgeClassName to the wrapper, not to the content", () => {
    const { container } = render(
      <CollapsibleSection
        title="Readable paths"
        badge={3}
        badgeClassName="collapsible__badge--accent"
      >
        x
      </CollapsibleSection>,
    );
    const badge = container.querySelector(".collapsible__badge")!;
    expect(badge).toHaveClass("collapsible__badge--accent");
    // El wrapper es el elemento con la clase; el contenido es texto plano.
    expect(badge.children).toHaveLength(0);
  });

  it("renders a count of zero", () => {
    // `badge != null` es la guarda: un conteo en cero tiene que verse igual
    // que cualquier otro, porque justamente el cero es el estado
    // informative (seccion sin paths configuradas).
    const { container } = render(
      <CollapsibleSection title="Writable paths" badge={0}>
        x
      </CollapsibleSection>,
    );
    expect(container.querySelector(".collapsible__badge")?.textContent).toBe("0");
  });

  it("renders the hint after the badge", () => {
    const { container } = render(
      <CollapsibleSection title="General" badge={2} hint="donde puede escribir">
        x
      </CollapsibleSection>,
    );
    const summary = container.querySelector(".collapsible__summary")!;
    const clases = [...summary.children].map((c) => c.className);
    expect(clases.indexOf("collapsible__badge")).toBeLessThan(
      clases.findIndex((c) => c.includes("collapsible__hint")),
    );
  });
});
