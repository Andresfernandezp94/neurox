import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { IconButton } from "./IconButton";

describe("IconButton", () => {
  it("renders a button with the icon", () => {
    const { container } = render(
      <IconButton icon="IconTrash" aria-label="Delete" />,
    );
    expect(screen.getByRole("button", { name: "Delete" })).toBeInTheDocument();
    expect(container.querySelector("svg")).toBeInTheDocument();
  });

  it("defaults to type=button", () => {
    render(<IconButton icon="IconTrash" aria-label="Delete" />);
    expect(screen.getByRole("button")).toHaveAttribute("type", "button");
  });

  it("calls onClick", () => {
    const handle = vi.fn();
    render(
      <IconButton
        icon="IconTrash"
        aria-label="Delete"
        onClick={handle}
      />,
    );
    fireEvent.click(screen.getByRole("button"));
    expect(handle).toHaveBeenCalledTimes(1);
  });

  it("supports danger variant", () => {
    render(
      <IconButton icon="IconTrash" aria-label="Delete" variant="danger" />,
    );
    expect(screen.getByRole("button")).toHaveClass("icon-btn--danger");
  });

  it("supports sm size", () => {
    render(
      <IconButton icon="IconTrash" aria-label="Delete" size="sm" />,
    );
    expect(screen.getByRole("button")).toHaveClass("icon-btn--sm");
  });

  it("respects disabled", () => {
    render(
      <IconButton icon="IconTrash" aria-label="Delete" disabled />,
    );
    expect(screen.getByRole("button")).toBeDisabled();
  });

  it("merges custom className", () => {
    render(
      <IconButton
        icon="IconTrash"
        aria-label="Delete"
        className="extra"
      />,
    );
    const btn = screen.getByRole("button");
    expect(btn).toHaveClass("extra");
    expect(btn).toHaveClass("icon-btn");
  });

  it("requires aria-label (TypeScript-enforced)", () => {
    // This test is a runtime check; the TS compile already enforces it.
    render(<IconButton icon="IconTrash" aria-label="Delete" />);
    expect(
      screen.getByRole("button", { name: "Delete" }),
    ).toBeInTheDocument();
  });
});
