import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { AlertModal } from "./AlertModal";

describe("AlertModal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <AlertModal open={false} message="Hello" onClose={() => {}} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders message when open", () => {
    render(<AlertModal open={true} message="Something happened" onClose={() => {}} />);
    expect(screen.getByText("Something happened")).toBeInTheDocument();
  });

  it("renders title when provided", () => {
    render(
      <AlertModal open={true} title="Warning" message="msg" onClose={() => {}} />,
    );
    expect(screen.getByText("Warning")).toBeInTheDocument();
  });

  it("calls onClose when OK button clicked", () => {
    const onClose = vi.fn();
    render(<AlertModal open={true} message="msg" onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("calls onClose on Escape keydown", () => {
    const onClose = vi.fn();
    render(<AlertModal open={true} message="msg" onClose={onClose} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("calls onClose on overlay click", () => {
    const onClose = vi.fn();
    render(<AlertModal open={true} message="msg" onClose={onClose} />);
    const overlay = document.querySelector(".alert-modal-overlay")!;
    fireEvent.mouseDown(overlay);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
