import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Code } from "./Code";

describe("Code", () => {
  it("renders children", () => {
    render(<Code>npm install</Code>);
    expect(screen.getByText("npm install")).toBeInTheDocument();
  });

  it("renders as <code> by default", () => {
    render(<Code>foo</Code>);
    expect(screen.getByText("foo").tagName).toBe("CODE");
  });

  it("supports other tags via `as`", () => {
    render(<Code as="span">foo</Code>);
    expect(screen.getByText("foo").tagName).toBe("SPAN");
  });

  it("applies text-mono class", () => {
    render(<Code>foo</Code>);
    expect(screen.getByText("foo")).toHaveClass("text-mono");
  });

  it("merges custom className", () => {
    render(<Code className="extra">foo</Code>);
    expect(screen.getByText("foo")).toHaveClass("extra");
  });
});
