import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SessionDetail } from "./SessionDetail";
import type { Message, SessionSummary } from "../../types";

const sampleMessages: Message[] = [
  {
    id: 1,
    session_id: "sess-1",
    role: "user",
    content: "Hello",
    ts: "2026-08-08T10:00:00Z",
  },
  {
    id: 2,
    session_id: "sess-1",
    role: "assistant",
    content: "Hi! How can I help?",
    ts: "2026-08-08T10:00:05Z",
    metrics: { durationMs: 5000, tokens: 12, tokensPerSec: 2.4 },
  },
  {
    id: 3,
    session_id: "sess-1",
    role: "tool",
    content: "shell: ls -la",
    ts: "2026-08-08T10:00:08Z",
  },
];

const sampleSession: SessionSummary = {
  session_id: "abcdef1234567890",
  agent_id: "default",
  started_at: "2026-08-08T10:00:00Z",
  summary: "Planning weekly tasks",
  ended_at: null,
};

describe("SessionDetail", () => {
  it("renders placeholder when sessionId is null", () => {
    render(<SessionDetail sessionId={null} messages={[]} />);
    expect(screen.getByText(/No session selected/i)).toBeInTheDocument();
    expect(screen.getByText(/Pick a session from the list/i)).toBeInTheDocument();
  });

  it("renders session summary as title when provided", () => {
    render(
      <SessionDetail
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={sampleMessages}
      />,
    );
    expect(screen.getByText("Planning weekly tasks")).toBeInTheDocument();
  });

  it("renders sessionId prefix when no summary", () => {
    render(
      <SessionDetail
        sessionId="abcdef1234567890"
        session={{ ...sampleSession, summary: null }}
        messages={[]}
      />,
    );
    expect(screen.getByText(/Session abcdef12…/)).toBeInTheDocument();
  });

  it("shows loading state", () => {
    render(<SessionDetail sessionId="abc" messages={[]} loading />);
    expect(screen.getByText(/Loading messages…/i)).toBeInTheDocument();
  });

  it("shows empty state when no messages", () => {
    render(<SessionDetail sessionId="abc" messages={[]} />);
    expect(screen.getByText(/No messages/i)).toBeInTheDocument();
  });

  it("renders all messages", () => {
    render(<SessionDetail sessionId="abc" messages={sampleMessages} />);
    expect(screen.getByText("Hello")).toBeInTheDocument();
    expect(screen.getByText("Hi! How can I help?")).toBeInTheDocument();
    expect(screen.getByText("shell: ls -la")).toBeInTheDocument();
  });

  it("renders metrics for messages that have them", () => {
    render(<SessionDetail sessionId="abc" messages={sampleMessages} />);
    expect(screen.getByText(/12t · 2.4\/s/)).toBeInTheDocument();
  });

  it("renders submeta with agent and id when session provided", () => {
    render(
      <SessionDetail
        sessionId="abcdef1234567890"
        session={sampleSession}
        messages={[]}
      />,
    );
    expect(screen.getByText("default")).toBeInTheDocument();
  });

  it("applies correct role-based styling class", () => {
    render(<SessionDetail sessionId="abc" messages={sampleMessages} />);
    expect(screen.getByText("Hello").closest(".session-detail__message")).toHaveClass(
      "session-detail__message--user",
    );
    expect(screen.getByText("Hi! How can I help?").closest(".session-detail__message")).toHaveClass(
      "session-detail__message--assistant",
    );
    expect(screen.getByText("shell: ls -la").closest(".session-detail__message")).toHaveClass(
      "session-detail__message--tool",
    );
  });
});
