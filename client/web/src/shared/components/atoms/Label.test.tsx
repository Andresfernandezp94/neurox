import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Label } from "./Label";

describe("Label", () => {
  it("renders children", () => {
    render(<Label>Username</Label>);
    expect(screen.getByText("Username")).toBeInTheDocument();
  });

  it("renders as a label element", () => {
    render(<Label>Username</Label>);
    expect(screen.getByText("Username").tagName).toBe("LABEL");
  });

  it("applies htmlFor", () => {
    render(<Label htmlFor="username-input">Username</Label>);
    const label = screen.getByText("Username");
    expect(label).toHaveAttribute("for", "username-input");
  });

  it("shows required indicator when required=true", () => {
    render(<Label required>Username</Label>);
    expect(screen.getByText("Username")).toHaveTextContent("Username *");
    expect(screen.getByText("*")).toHaveAttribute("aria-hidden", "true");
  });

  it("does not show required indicator by default", () => {
    render(<Label>Username</Label>);
    expect(screen.queryByText("*")).not.toBeInTheDocument();
  });

  it("applies label-text class", () => {
    render(<Label>Username</Label>);
    expect(screen.getByText("Username")).toHaveClass("label-text");
  });

  it("applies label-text--required when required", () => {
    render(<Label required>Username</Label>);
    expect(screen.getByText("Username")).toHaveClass("label-text--required");
  });

  it("merges custom className", () => {
    render(<Label className="extra">Username</Label>);
    expect(screen.getByText("Username")).toHaveClass("extra");
  });
});
