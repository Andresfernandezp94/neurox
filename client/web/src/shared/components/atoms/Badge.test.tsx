import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Badge } from "./Badge";

describe("Badge", () => {
  it("renders children", () => {
    render(<Badge>Active</Badge>);
    expect(screen.getByText("Active")).toBeInTheDocument();
  });

  it("defaults to neutral variant", () => {
    render(<Badge>Active</Badge>);
    expect(screen.getByText("Active")).toHaveClass("badge");
    expect(screen.getByText("Active")).not.toHaveClass("badge--success");
  });

  it("applies variant class for success", () => {
    render(<Badge variant="success">Active</Badge>);
    expect(screen.getByText("Active")).toHaveClass("badge--success");
  });

  it("supports all variants", () => {
    const variants = ["neutral", "success", "warn", "danger", "info"] as const;
    for (const v of variants) {
      const { unmount } = render(<Badge variant={v}>{v}</Badge>);
      const el = screen.getByText(v);
      if (v === "neutral") {
        expect(el).toHaveClass("badge");
      } else {
        expect(el).toHaveClass(`badge--${v}`);
      }
      unmount();
    }
  });

  it("renders dot when dot=true", () => {
    const { container } = render(<Badge dot>Active</Badge>);
    const dot = container.querySelector(".badge__dot");
    expect(dot).toBeInTheDocument();
    expect(dot).toHaveAttribute("aria-hidden", "true");
  });

  it("does not render dot by default", () => {
    const { container } = render(<Badge>Active</Badge>);
    expect(container.querySelector(".badge__dot")).not.toBeInTheDocument();
  });

  it("merges custom className", () => {
    render(<Badge className="extra">Active</Badge>);
    expect(screen.getByText("Active")).toHaveClass("extra");
  });
});
