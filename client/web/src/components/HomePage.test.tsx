import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { HomePage } from "./HomePage";

// Landing PÚBLICA: no lee el store ni hace fetch. Por eso este test no
// mockea StoreContext — si la landing volviera a depender del store,
// estos tests empezarían a fallar y eso es justamente la señal que
// queremos: la regla "landing sin datos internos" queda vigilada.

describe("HomePage (landing pública)", () => {
  it("renders the landing page", () => {
    render(<HomePage />);
    expect(screen.getByTestId("home")).toBeInTheDocument();
  });

  it("shows the neurox wordmark in the header brand", () => {
    // Con el hero oculto, el único "neurox" visible es el del brand del
    // header. Antes venía también del <h1> del hero.
    const { container } = render(<HomePage />);
    expect(container.querySelector(".home__brand-name")).toHaveTextContent(
      "neurox",
    );
  });

  it("has the landing structure: header, main, footer", () => {
    const { container } = render(<HomePage />);
    expect(container.querySelector(".home__header")).toBeInTheDocument();
    expect(container.querySelector(".home__main")).toBeInTheDocument();
    expect(container.querySelector(".home__footer")).toBeInTheDocument();
    // <main> semántico, no un div.
    expect(container.querySelector("main.home__main")).toBeInTheDocument();
  });

  it("does not render the hero (hidden for now)", () => {
    const { container } = render(<HomePage />);
    expect(container.querySelector(".home__hero")).not.toBeInTheDocument();
  });

  it("presents the product capabilities", () => {
    render(<HomePage />);
    expect(screen.getByText("Agentes por sesión")).toBeInTheDocument();
    expect(screen.getByText("Tools engine")).toBeInTheDocument();
    expect(screen.getByText("Plugins y MCP")).toBeInTheDocument();
    expect(screen.getByText("Proveedores LLM")).toBeInTheDocument();
  });

  it("does NOT leak internal metrics before login", () => {
    // Regresión: la landing no debe renderizar las STATS del panel
    // (latencia / agentes / sesiones) antes de iniciar sesión.
    //
    // Ojo: las palabras "agentes" y "sesiones" sí aparecen en los textos
    // descriptivos de capacidades — eso es contenido editorial, no un
    // valor. Lo que se verifica es que no exista el bloque de stats.
    const { container } = render(<HomePage />);
    expect(container.querySelector(".home__stats")).not.toBeInTheDocument();
    expect(container.querySelector(".home__stat")).not.toBeInTheDocument();
    expect(screen.queryByText("latencia (ms)")).not.toBeInTheDocument();
  });

  it("omits the version when not provided", () => {
    render(<HomePage />);
    expect(screen.queryByText(/^v\d/)).not.toBeInTheDocument();
  });

  it("shows the version in the footer when provided", () => {
    render(<HomePage version="0.4.0" />);
    expect(screen.getByText("v0.4.0")).toBeInTheDocument();
  });

  it("hides the entry button when onEnter is not provided", () => {
    render(<HomePage />);
    expect(screen.queryByTestId("home-enter")).not.toBeInTheDocument();
  });

  it("calls onEnter from the header button", () => {
    const onEnter = vi.fn();
    render(<HomePage onEnter={onEnter} />);
    screen.getByTestId("home-enter").click();
    expect(onEnter).toHaveBeenCalledTimes(1);
  });

  });