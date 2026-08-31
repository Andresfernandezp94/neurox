import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { SessionDetailModal } from "./SessionDetailModal";
import type { Message, SessionSummary } from "../../types";

const sampleMessages: Message[] = [
  {
    id: 1,
    session_id: "sess-1",
    role: "user",
    content: "Hello",
    ts: "2026-08-08T10:00:00Z",
  },
];

const sampleSession: SessionSummary = {
  session_id: "abcdef1234567890",
  agent_id: "default",
  started_at: "2026-08-08T10:00:00Z",
  summary: "Planning",
  ended_at: null,
};

describe("SessionDetailModal", () => {
  it("renders nothing when sessionId is null", () => {
    const { container } = render(
      <SessionDetailModal sessionId={null} messages={[]} onClose={vi.fn()} />,
    );
    expect(container.querySelector(".session-detail-modal")).toBeNull();
  });

  it("renders modal when sessionId is provided", () => {
    render(
      <SessionDetailModal
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={sampleMessages}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Planning")).toBeInTheDocument();
    expect(screen.getByText("Hello")).toBeInTheDocument();
  });

  it("calls onClose when ✕ button clicked", () => {
    const onClose = vi.fn();
    render(
      <SessionDetailModal
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={[]}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText(/Close/i));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("calls onClose when backdrop clicked", () => {
    const onClose = vi.fn();
    render(
      <SessionDetailModal
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={[]}
        onClose={onClose}
      />,
    );
    const backdrop = screen.getByRole("dialog");
    fireEvent.click(backdrop);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("does NOT call onClose when clicking inside the container", () => {
    const onClose = vi.fn();
    render(
      <SessionDetailModal
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={sampleMessages}
        onClose={onClose}
      />,
    );
    // Click on the message content (inside container, not backdrop)
    fireEvent.click(screen.getByText("Hello"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("calls onClose when Escape key is pressed", () => {
    const onClose = vi.fn();
    render(
      <SessionDetailModal
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={[]}
        onClose={onClose}
      />,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("renders placeholder when sessionId is null but with null safety", () => {
    const { container } = render(
      <SessionDetailModal sessionId={null} messages={[]} onClose={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });
});
