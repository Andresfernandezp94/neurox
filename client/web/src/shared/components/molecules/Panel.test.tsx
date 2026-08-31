import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Panel } from "./Panel";

describe("Panel", () => {
  it("renders children", () => {
    render(<Panel>content</Panel>);
    expect(screen.getByText("content")).toBeInTheDocument();
  });

  it("renders title in header", () => {
    render(<Panel title="Health">body</Panel>);
    const title = screen.getByText("Health");
    expect(title).toHaveClass("panel__title");
    expect(title.parentElement).toHaveClass("panel__header");
  });

  it("renders actions in header", () => {
    render(
      <Panel actions={<button>Restart</button>}>body</Panel>,
    );
    const actions = screen.getByRole("button").parentElement;
    expect(actions).toHaveClass("panel__actions");
  });

  it("renders body", () => {
    render(<Panel>body content</Panel>);
    const body = screen.getByText("body content");
    expect(body).toHaveClass("panel__body");
  });

  it("does not render header when no title or actions", () => {
    const { container } = render(<Panel>body</Panel>);
    expect(container.querySelector(".panel__header")).not.toBeInTheDocument();
  });

  it("applies panel class", () => {
    const { container } = render(<Panel>body</Panel>);
    expect(container.firstChild).toHaveClass("panel");
  });

  it("supports polymorphic root", () => {
    const { container } = render(<Panel as="article">body</Panel>);
    expect((container.firstChild as HTMLElement | null)?.tagName).toBe("ARTICLE");
  });

  it("merges custom className", () => {
    const { container } = render(
      <Panel className="extra">body</Panel>,
    );
    expect(container.firstChild).toHaveClass("extra");
  });
});
