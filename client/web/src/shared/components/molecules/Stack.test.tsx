import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Stack } from "./Stack";

describe("Stack", () => {
  it("renders children", () => {
    render(<Stack>A</Stack>);
    expect(screen.getByText("A")).toBeInTheDocument();
  });

  it("applies stack class", () => {
    const { container } = render(<Stack>x</Stack>);
    expect(container.firstChild).toHaveClass("stack");
  });

  it("defaults to column direction", () => {
    const { container } = render(<Stack>x</Stack>);
    expect(container.firstChild).not.toHaveClass("stack--row");
  });

  it("applies row direction", () => {
    const { container } = render(<Stack direction="row">x</Stack>);
    expect(container.firstChild).toHaveClass("stack--row");
  });

  it("applies gap class", () => {
    const { container } = render(<Stack gap="lg">x</Stack>);
    expect(container.firstChild).toHaveClass("stack--gap-lg");
  });

  it("does not apply gap class for gap=none", () => {
    const { container } = render(<Stack gap="none">x</Stack>);
    expect(container.firstChild).not.toHaveClass("stack--gap");
  });

  it("applies align class when provided", () => {
    const { container } = render(<Stack align="center">x</Stack>);
    expect(container.firstChild).toHaveClass("stack--align-center");
  });

  it("merges custom className", () => {
    const { container } = render(<Stack className="extra">x</Stack>);
    expect(container.firstChild).toHaveClass("extra");
  });
});
