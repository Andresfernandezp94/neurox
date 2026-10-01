import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { GridBackdrop } from "./GridBackdrop";

describe("GridBackdrop", () => {
  it("renders the backdrop and its lines layer", () => {
    const { container } = render(<GridBackdrop />);
    expect(container.querySelector(".grid-backdrop")).toBeInTheDocument();
    expect(container.querySelector(".grid-backdrop__lines")).toBeInTheDocument();
  });

  it("is hidden from assistive tech and never intercepts clicks", () => {
    // Decorativo: si no, un lector de pantalla anunciaría un div vacío
    // y la grilla bloquearía los clicks del contenido de encima.
    const { container } = render(<GridBackdrop />);
    const backdrop = container.querySelector(".grid-backdrop");
    expect(backdrop).toHaveAttribute("aria-hidden", "true");
  });

  it("exposes the grid geometry as CSS custom properties", () => {
    // Parametrizar por CSS vars es lo que permite compartir el átomo
    // entre login y landing sin duplicar la regla de background-image.
    const { container } = render(<GridBackdrop cellSize="5rem" fadeEnd={90} />);
    const lines = container.querySelector<HTMLElement>(
      ".grid-backdrop__lines",
    )!;
    expect(lines.style.getPropertyValue("--grid-cell-size")).toBe("5rem");
    expect(lines.style.getPropertyValue("--grid-fade-end")).toBe("90%");
  });

  it("uses accent-derived defaults so it follows the theme", () => {
    const { container } = render(<GridBackdrop />);
    const lines = container.querySelector<HTMLElement>(
      ".grid-backdrop__lines",
    )!;
    const color = lines.style.getPropertyValue("--grid-line-color");
    expect(color).toContain("var(--accent)");
  });

  it("accepts a custom line color", () => {
    const { container } = render(<GridBackdrop lineColor="rgba(1, 2, 3, 0.2)" />);
    const lines = container.querySelector<HTMLElement>(
      ".grid-backdrop__lines",
    )!;
    expect(lines.style.getPropertyValue("--grid-line-color")).toBe(
      "rgba(1, 2, 3, 0.2)",
    );
  });

  it("exposes the line thickness as a CSS custom property", () => {
    // El grosor se aplicaba hardcodeado en el gradiente; ahora es una
    // variable, así se puede hacer más gruesa sin tocar el CSS.
    const { container, rerender } = render(<GridBackdrop />);
    const lines = container.querySelector<HTMLElement>(
      ".grid-backdrop__lines",
    )!;
    expect(lines.style.getPropertyValue("--grid-line-width")).toBe("2px");

    rerender(<GridBackdrop lineWidth="4px" />);
    expect(lines.style.getPropertyValue("--grid-line-width")).toBe("4px");
  });

  it("merges a custom className", () => {
    const { container } = render(<GridBackdrop className="home__grid" />);
    const backdrop = container.querySelector(".grid-backdrop");
    expect(backdrop?.className).toContain("home__grid");
  });

  it("adds the fixed modifier when fixed is set", () => {
    // El fondo global de la app usa una sola instancia fija al viewport,
    // compartida por landing, login y panel.
    const { container } = render(<GridBackdrop fixed />);
    const backdrop = container.querySelector(".grid-backdrop");
    expect(backdrop?.className).toContain("grid-backdrop--fixed");
  });
});