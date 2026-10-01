import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Card } from "./Card";

describe("Card", () => {
  it("renders children", () => {
    render(<Card>Hello</Card>);
    expect(screen.getByText("Hello")).toBeInTheDocument();
  });

  it("applies card class", () => {
    const { container } = render(<Card>Hello</Card>);
    expect(container.firstChild).toHaveClass("card");
  });

  it("renders Title sub-component", () => {
    render(
      <Card>
        <Card.Title>Title</Card.Title>
      </Card>,
    );
    const title = screen.getByText("Title");
    expect(title).toHaveClass("card__title");
  });

  it("renders Body sub-component", () => {
    render(
      <Card>
        <Card.Body>Body</Card.Body>
      </Card>,
    );
    const body = screen.getByText("Body");
    expect(body).toHaveClass("card__body");
  });

  it("renders Actions sub-component", () => {
    render(
      <Card>
        <Card.Actions>
          <button>Save</button>
        </Card.Actions>
      </Card>,
    );
    expect(screen.getByText("Save").parentElement).toHaveClass(
      "card__actions",
    );
  });

  it("supports polymorphic root via `as`", () => {
    render(
      <Card as="section">
        <Card.Title>S</Card.Title>
      </Card>,
    );
    const card = screen.getByText("S").parentElement;
    expect(card?.tagName).toBe("SECTION");
  });

  it("merges custom className", () => {
    const { container } = render(
      <Card className="extra">Hello</Card>,
    );
    expect(container.firstChild).toHaveClass("extra");
    expect(container.firstChild).toHaveClass("card");
  });

  // Regresión: gapClass hacía que `sm` devolviera sólo `stack--gap-sm`,
  // sin `stack`. Sin `stack` la card queda en el display:block default
  // de `.card` y `gap` no aplica (gap no existe en block), así que los
  // hijos se veían pegados. Cada variante con gap debe traer `stack`.
  it.each(["sm", "md", "lg"] as const)(
    "gap=%s implies a flex container so gap actually applies",
    (gap) => {
      const { container } = render(<Card gap={gap}>x</Card>);
      const card = container.firstChild;
      expect(card).toHaveClass("stack");
      expect(card).toHaveClass(`stack--gap-${gap}`);
    },
  );

  it("gap=none leaves the card as a plain block", () => {
    // Único caso sin `stack`: el llamador quiere el block plano de .card.
    const { container } = render(<Card gap="none">x</Card>);
    expect(container.firstChild).not.toHaveClass("stack");
    expect(container.firstChild).toHaveClass("card");
  });

  it("defaults to md", () => {
    const { container } = render(<Card>x</Card>);
    expect(container.firstChild).toHaveClass("stack--gap-md");
  });

  it("renders full compound tree", () => {
    render(
      <Card>
        <Card.Title>Service Health</Card.Title>
        <Card.Body>OK</Card.Body>
        <Card.Actions>
          <button>Restart</button>
        </Card.Actions>
      </Card>,
    );
    expect(screen.getByText("Service Health")).toHaveClass("card__title");
    expect(screen.getByText("OK")).toHaveClass("card__body");
    expect(screen.getByRole("button")).toBeInTheDocument();
  });
});
