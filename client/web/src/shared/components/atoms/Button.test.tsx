import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Button } from "./Button";

describe("Button", () => {
  it("renders children", () => {
    render(<Button>Save</Button>);
    expect(screen.getByRole("button", { name: "Save" })).toBeInTheDocument();
  });

  it("defaults to type=button", () => {
    render(<Button>Save</Button>);
    const btn = screen.getByRole("button");
    expect(btn).toHaveAttribute("type", "button");
  });

  it("applies variant class", () => {
    render(<Button variant="primary">Save</Button>);
    expect(screen.getByRole("button")).toHaveClass("btn--primary");
  });

  it("applies size class for sm", () => {
    render(<Button size="sm">Save</Button>);
    expect(screen.getByRole("button")).toHaveClass("btn--sm");
  });

  it("does not apply size class for md (default)", () => {
    render(<Button size="md">Save</Button>);
    expect(screen.getByRole("button")).not.toHaveClass("btn--sm");
  });

  it("merges custom className", () => {
    render(<Button className="extra">Save</Button>);
    expect(screen.getByRole("button")).toHaveClass("extra");
    expect(screen.getByRole("button")).toHaveClass("btn--secondary");
  });

  it("calls onClick when clicked", () => {
    const handle = vi.fn();
    render(<Button onClick={handle}>Save</Button>);
    fireEvent.click(screen.getByRole("button"));
    expect(handle).toHaveBeenCalledTimes(1);
  });

  it("does not call onClick when disabled", () => {
    const handle = vi.fn();
    render(
      <Button onClick={handle} disabled>
        Save
      </Button>,
    );
    fireEvent.click(screen.getByRole("button"));
    expect(handle).not.toHaveBeenCalled();
  });

  it("does not call onClick when loading", () => {
    const handle = vi.fn();
    render(
      <Button onClick={handle} loading>
        Save
      </Button>,
    );
    fireEvent.click(screen.getByRole("button"));
    expect(handle).not.toHaveBeenCalled();
  });

  it("sets aria-busy when loading", () => {
    render(<Button loading>Save</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("aria-busy", "true");
  });

  it("does not set aria-busy when not loading", () => {
    render(<Button>Save</Button>);
    expect(screen.getByRole("button")).not.toHaveAttribute("aria-busy");
  });

  it("forwards disabled to underlying button", () => {
    render(<Button disabled>Save</Button>);
    expect(screen.getByRole("button")).toBeDisabled();
  });

  it("supports all variants", () => {
    const variants = ["primary", "secondary", "danger", "ghost"] as const;
    for (const v of variants) {
      const { unmount } = render(<Button variant={v}>{v}</Button>);
      expect(screen.getByRole("button")).toHaveClass(`btn--${v}`);
      unmount();
    }
  });
});
