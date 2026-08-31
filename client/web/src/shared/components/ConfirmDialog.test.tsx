import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

describe("ConfirmDialog", () => {
  const baseProps = {
    open: true,
    title: "Confirm",
    message: "Are you sure?",
    onConfirm: vi.fn(),
    onCancel: vi.fn(),
  };

  it("renders nothing when closed", () => {
    const { container } = render(
      <ConfirmDialog {...baseProps} open={false} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders title and message when open", () => {
    render(<ConfirmDialog {...baseProps} />);
    expect(screen.getByText("Confirm")).toBeInTheDocument();
    expect(screen.getByText("Are you sure?")).toBeInTheDocument();
  });

  it("calls onConfirm on confirm button click", () => {
    const onConfirm = vi.fn();
    render(<ConfirmDialog {...baseProps} onConfirm={onConfirm} />);
    fireEvent.click(screen.getByTestId("confirm-dialog-confirm"));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("calls onCancel on cancel button click", () => {
    const onCancel = vi.fn();
    render(<ConfirmDialog {...baseProps} onCancel={onCancel} />);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onCancel on Escape keydown", () => {
    const onCancel = vi.fn();
    render(<ConfirmDialog {...baseProps} onCancel={onCancel} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onCancel on overlay click", () => {
    const onCancel = vi.fn();
    const { container } = render(
      <ConfirmDialog {...baseProps} onCancel={onCancel} />,
    );
    const overlay = container.firstChild as HTMLElement;
    fireEvent.mouseDown(overlay);
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("uses custom labels", () => {
    render(
      <ConfirmDialog
        {...baseProps}
        confirmLabel="Delete"
        cancelLabel="Abort"
      />,
    );
    expect(screen.getByRole("button", { name: "Abort" })).toBeInTheDocument();
    expect(screen.getByText("Delete")).toBeInTheDocument();
  });

  it("applies destructive variant", () => {
    render(<ConfirmDialog {...baseProps} destructive />);
    const btn = screen.getByTestId("confirm-dialog-confirm");
    expect(btn.className).toContain("btn--danger");
  });
});
