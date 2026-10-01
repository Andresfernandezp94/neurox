import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { BootLoader } from "./BootLoader";

describe("BootLoader", () => {
  it("renders the loader stage", () => {
    render(<BootLoader />);
    expect(screen.getByTestId("boot-loader")).toBeInTheDocument();
  });

  it("renders the neurox svg", () => {
    const { container } = render(<BootLoader />);
    const svg = container.querySelector("svg.boot-loader__logo");
    expect(svg).toBeInTheDocument();
  });

  it("renders the wordmark", () => {
    render(<BootLoader />);
    expect(container_text()).toContain("neurox");
  });

  it("omits the status line when no status prop is given", () => {
    render(<BootLoader />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders the status line when given", () => {
    render(<BootLoader status="conectando con el daemon" />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "conectando con el daemon",
    );
  });

  it("keeps the decorative parts hidden from assistive tech", () => {
    // El loader es decorativo: el logo y el wordmark van aria-hidden,
    // sólo el status se anuncia.
    const { container } = render(<BootLoader status="cargando" />);
    expect(container.querySelector(".boot-loader__logo")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(container.querySelector(".boot-loader__wordmark")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });
});

// helper local para no repetir container plumbing
function container_text(): string {
  return document.body.textContent ?? "";
}