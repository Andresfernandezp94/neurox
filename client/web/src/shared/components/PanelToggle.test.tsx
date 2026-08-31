// PanelToggle — convenience wrapper around useHiddenPanels.

import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PanelToggle } from "./PanelToggle";

// Mock useHiddenPanels so each test controls what's hidden.
const mockIsHidden = vi.fn();
vi.mock("../../hooks/useHiddenPanels", () => ({
  useHiddenPanels: () => ({ isHidden: mockIsHidden, hidden: new Set() }),
}));

function setHidden(value: boolean) {
  mockIsHidden.mockReset();
  mockIsHidden.mockReturnValue(value);
}

describe("PanelToggle", () => {
  beforeEach(() => {
    setHidden(false);
  });

  afterEach(() => {
    mockIsHidden.mockReset();
  });

  it("renders children when the panel is NOT hidden", () => {
    setHidden(false);
    render(
      <PanelToggle id="contextBar">
        <div data-testid="child">Hello</div>
      </PanelToggle>,
    );
    expect(screen.getByTestId("child")).toBeInTheDocument();
    expect(screen.getByText("Hello")).toBeInTheDocument();
  });

  it("renders nothing when the panel IS hidden", () => {
    setHidden(true);
    const { container } = render(
      <PanelToggle id="contextBar">
        <div data-testid="child">Hello</div>
      </PanelToggle>,
    );
    expect(screen.queryByTestId("child")).not.toBeInTheDocument();
    expect(container.firstChild).toBeNull();
  });

  it("forwards the id to isHidden", () => {
    setHidden(false);
    render(
      <PanelToggle id="chat-history">
        <div data-testid="child">x</div>
      </PanelToggle>,
    );
    expect(mockIsHidden).toHaveBeenCalledWith("chat-history");
  });
});