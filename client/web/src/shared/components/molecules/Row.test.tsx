import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Row } from "./Row";

describe("Row", () => {
  it("renders children", () => {
    render(
      <Row>
        <span>A</span>
        <span>B</span>
      </Row>,
    );
    expect(screen.getByText("A")).toBeInTheDocument();
    expect(screen.getByText("B")).toBeInTheDocument();
  });

  it("applies row class", () => {
    const { container } = render(<Row>x</Row>);
    expect(container.firstChild).toHaveClass("row");
  });

  it("applies justify=between", () => {
    const { container } = render(<Row justify="between">x</Row>);
    expect(container.firstChild).toHaveClass("row--between");
  });

  it("applies align=center by default", () => {
    const { container } = render(<Row>x</Row>);
    expect(container.firstChild).toHaveClass("row--align-center");
  });

  it("applies gap class", () => {
    const { container } = render(<Row gap="lg">x</Row>);
    expect(container.firstChild).toHaveClass("row--gap-lg");
  });

  it("does not apply gap class for gap=none", () => {
    const { container } = render(<Row gap="none">x</Row>);
    expect(container.firstChild).not.toHaveClass("row--gap");
  });

  it("applies wrap modifier when wrap=true", () => {
    const { container } = render(<Row wrap>x</Row>);
    expect(container.firstChild).toHaveClass("row--wrap");
  });

  it("merges custom className", () => {
    const { container } = render(<Row className="extra">x</Row>);
    expect(container.firstChild).toHaveClass("extra");
  });
});
