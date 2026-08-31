import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SectionHeader } from "./SectionHeader";

describe("SectionHeader", () => {
  it("renders title", () => {
    render(<SectionHeader title="My Title" />);
    expect(screen.getByText("My Title")).toBeInTheDocument();
  });

  it("renders description when provided", () => {
    render(<SectionHeader title="T" description="Some desc" />);
    expect(screen.getByText("Some desc")).toBeInTheDocument();
  });

  it("does not render description when omitted", () => {
    const { container } = render(<SectionHeader title="T" />);
    expect(container.querySelectorAll("p")).toHaveLength(0);
  });

  it("renders actions slot", () => {
    render(
      <SectionHeader title="T" actions={<button>Click</button>} />,
    );
    expect(screen.getByRole("button", { name: "Click" })).toBeInTheDocument();
  });
});
