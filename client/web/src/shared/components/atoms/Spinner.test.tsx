import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Spinner } from "./Spinner";

describe("Spinner", () => {
  it("renders with role=status", () => {
    render(<Spinner />);
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  it("uses default label when not provided", () => {
    render(<Spinner />);
    expect(screen.getByRole("status")).toHaveAttribute("aria-label", "Loading");
  });

  it("uses custom label", () => {
    render(<Spinner label="Fetching sessions" />);
    expect(screen.getByRole("status")).toHaveAttribute(
      "aria-label",
      "Fetching sessions",
    );
  });

  it("applies size class", () => {
    const { rerender } = render(<Spinner size="sm" />);
    expect(screen.getByRole("status")).toHaveClass("spinner--sm");

    rerender(<Spinner size="lg" />);
    expect(screen.getByRole("status")).toHaveClass("spinner--lg");
  });

  it("applies default size class", () => {
    render(<Spinner />);
    expect(screen.getByRole("status")).toHaveClass("spinner--md");
  });

  it("merges custom className", () => {
    render(<Spinner className="extra" />);
    expect(screen.getByRole("status")).toHaveClass("extra");
    expect(screen.getByRole("status")).toHaveClass("spinner");
  });
});
