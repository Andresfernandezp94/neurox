import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("renders children", () => {
    render(<EmptyState>content</EmptyState>);
    expect(screen.getByText("content")).toBeInTheDocument();
  });

  it("applies empty class", () => {
    const { container } = render(<EmptyState>x</EmptyState>);
    expect(container.firstChild).toHaveClass("empty");
  });

  it("renders Title sub-component", () => {
    render(
      <EmptyState>
        <EmptyState.Title>No items</EmptyState.Title>
      </EmptyState>,
    );
    expect(screen.getByText("No items")).toHaveClass("empty__title");
  });

  it("renders Hint sub-component", () => {
    render(
      <EmptyState>
        <EmptyState.Hint>Try adjusting your filters</EmptyState.Hint>
      </EmptyState>,
    );
    expect(screen.getByText("Try adjusting your filters")).toHaveClass(
      "empty__hint",
    );
  });

  it("renders Icon sub-component", () => {
    render(
      <EmptyState>
        <EmptyState.Icon>📭</EmptyState.Icon>
      </EmptyState>,
    );
    expect(screen.getByText("📭")).toHaveClass("empty__icon");
  });

  it("renders full compound tree", () => {
    render(
      <EmptyState>
        <EmptyState.Icon>📭</EmptyState.Icon>
        <EmptyState.Title>Nothing here</EmptyState.Title>
        <EmptyState.Hint>Add an item to get started</EmptyState.Hint>
      </EmptyState>,
    );
    expect(screen.getByText("📭")).toHaveClass("empty__icon");
    expect(screen.getByText("Nothing here")).toHaveClass("empty__title");
    expect(screen.getByText("Add an item to get started")).toHaveClass(
      "empty__hint",
    );
  });

  it("merges custom className", () => {
    const { container } = render(
      <EmptyState className="extra">x</EmptyState>,
    );
    expect(container.firstChild).toHaveClass("extra");
  });
});
