import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatPair } from "./StatPair";

describe("StatPair", () => {
  it("renders label and value", () => {
    render(<StatPair label="Total" value={42} />);
    expect(screen.getByText("Total")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
  });

  it("applies default stat-pair class", () => {
    const { container } = render(<StatPair label="x" value={1} />);
    expect(container.firstChild).toHaveClass("stat-pair");
  });

  it("applies variant=total class", () => {
    const { container } = render(
      <StatPair label="x" value={1} variant="total" />,
    );
    expect(container.firstChild).toHaveClass("stat-pair--total");
  });

  it("applies variant=active class", () => {
    const { container } = render(
      <StatPair label="x" value={1} variant="active" />,
    );
    expect(container.firstChild).toHaveClass("stat-pair--active");
  });

  it("applies variant=closed class", () => {
    const { container } = render(
      <StatPair label="x" value={1} variant="closed" />,
    );
    expect(container.firstChild).toHaveClass("stat-pair--closed");
  });

  it("supports ReactNode label", () => {
    render(
      <StatPair
        label={<strong>Bold label</strong>}
        value={1}
      />,
    );
    expect(screen.getByText("Bold label").tagName).toBe("STRONG");
  });

  it("supports ReactNode value", () => {
    render(
      <StatPair
        label="x"
        value={<span data-testid="custom-value">123</span>}
      />,
    );
    expect(screen.getByTestId("custom-value")).toBeInTheDocument();
  });

  it("merges custom className", () => {
    const { container } = render(
      <StatPair label="x" value={1} className="extra" />,
    );
    expect(container.firstChild).toHaveClass("extra");
  });
});
