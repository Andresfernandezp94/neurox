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
