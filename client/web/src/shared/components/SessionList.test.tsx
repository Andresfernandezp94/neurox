import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { SessionList } from "./SessionList";
import type { SessionSummary } from "../../types";

const baseSessions: SessionSummary[] = [
  {
    session_id: "sess-12345678-abcdef",
    agent_id: "default",
    started_at: "2026-08-08T10:00:00Z",
    summary: "Planning weekly tasks",
    ended_at: null,
  },
  {
    session_id: "sess-99999999-xyz",
    agent_id: "default",
    started_at: "2026-08-07T15:00:00Z",
    summary: "Debugging memory leak",
    ended_at: "2026-08-07T16:00:00Z",
  },
  {
    session_id: "sess-aaaaaa-noend",
    agent_id: "researcher",
    started_at: "2026-08-08T11:00:00Z",
    summary: null,
    ended_at: null,
  },
];

describe("SessionList", () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it("renders sidebar variant with default header 'History'", () => {
    render(<SessionList sessions={baseSessions} variant="sidebar" />);
    expect(screen.getByText("History")).toBeInTheDocument();
  });

  it("renders panel variant", () => {
    render(<SessionList sessions={baseSessions} variant="panel" />);
    expect(screen.getByText("Sessions")).toBeInTheDocument();
  });

  it("renders expanded variant with stats", () => {
    render(<SessionList sessions={baseSessions} variant="expanded" />);
    expect(screen.getByText("Sessions")).toBeInTheDocument();
    expect(screen.getByText(/3 total/i)).toBeInTheDocument();
    expect(screen.getByText(/2 active/i)).toBeInTheDocument();
    expect(screen.getByText(/1 closed/i)).toBeInTheDocument();
  });

  it("shows empty state when no sessions", () => {
    render(<SessionList sessions={[]} variant="sidebar" />);
    expect(screen.getByText(/No previous chats/i)).toBeInTheDocument();
  });

  it("shows empty state for expanded variant", () => {
    render(<SessionList sessions={[]} variant="expanded" />);
    expect(screen.getByText(/No sessions yet/i)).toBeInTheDocument();
  });

  it("shows loading state when loading and no sessions", () => {
    render(<SessionList sessions={[]} loading variant="sidebar" />);
    expect(screen.getByText(/Loading/i)).toBeInTheDocument();
  });

  it("shows empty match state when search has no results", () => {
    // EP-0028-b: the sidebar variant hides the search input — the
    // search/filter behaviour is exercised here against the panel
    // variant, where the input is still rendered.
    render(<SessionList sessions={baseSessions} variant="panel" />);
    const input = screen.getByPlaceholderText(/Search sessions/i);
    fireEvent.change(input, { target: { value: "nothing-matches" } });
    expect(screen.getByText(/No matches for/i)).toBeInTheDocument();
  });

  it("filters by summary text (case-insensitive)", () => {
    render(<SessionList sessions={baseSessions} variant="panel" />);
    const input = screen.getByPlaceholderText(/Search sessions/i);
    fireEvent.change(input, { target: { value: "DEBUGGING" } });
    expect(screen.getByText(/Debugging memory leak/i)).toBeInTheDocument();
    expect(screen.queryByText(/Planning weekly tasks/i)).not.toBeInTheDocument();
  });

  it("filters by session_id", () => {
    render(<SessionList sessions={baseSessions} variant="panel" />);
    const input = screen.getByPlaceholderText(/Search sessions/i);
    fireEvent.change(input, { target: { value: "99999999" } });
    expect(screen.getByText(/Debugging memory leak/i)).toBeInTheDocument();
  });

  it("filters by agent_id", () => {
    render(<SessionList sessions={baseSessions} variant="panel" />);
    const input = screen.getByPlaceholderText(/Search sessions/i);
    fireEvent.change(input, { target: { value: "researcher" } });
    // The session matching "researcher" has no summary so its
    // title falls back to the truncated session_id ("sess-aaaa…").
    // The agent_id "researcher" itself is shown in the meta row.
    expect(screen.getByText("researcher")).toBeInTheDocument();
  });

  it("clears search with × button", () => {
    render(<SessionList sessions={baseSessions} variant="panel" />);
    const input = screen.getByPlaceholderText(/Search sessions/i) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "planning" } });
    expect(input.value).toBe("planning");
    const clearBtn = screen.getByLabelText("Clear search");
    fireEvent.click(clearBtn);
    expect(input.value).toBe("");
  });

  it("calls onLoad when an item is clicked", () => {
    const onLoad = vi.fn();
    render(<SessionList sessions={baseSessions} variant="sidebar" onLoad={onLoad} />);
    fireEvent.click(screen.getByText(/Planning weekly tasks/i));
    expect(onLoad).toHaveBeenCalledWith("sess-12345678-abcdef");
  });

  it("highlights the current session", () => {
    render(
      <SessionList
        sessions={baseSessions}
        currentSessionId="sess-12345678-abcdef"
        variant="sidebar"
      />,
    );
    const item = screen.getByTestId("session-list-item-sess-12345678-abcdef");
    expect(item.className).toContain("session-list__item--active");
  });

  it("does not render rename/delete buttons when handlers not provided", () => {
    render(<SessionList sessions={baseSessions} variant="panel" />);
    expect(screen.queryByLabelText("Rename session")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Delete session")).not.toBeInTheDocument();
  });

  it("enters rename mode on ✎ click and submits on Enter", async () => {
    const onRename = vi.fn().mockResolvedValue(undefined);
    render(
      <SessionList sessions={baseSessions} variant="panel" onRename={onRename} />,
    );
    const renameBtn = screen.getByTestId("session-list-rename-sess-12345678-abcdef");
    fireEvent.click(renameBtn);
    const input = screen.getByTestId("session-list-rename-input") as HTMLInputElement;
    expect(input).toBeInTheDocument();
    fireEvent.change(input, { target: { value: "New title" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(onRename).toHaveBeenCalledWith("sess-12345678-abcdef", "New title");
    });
  });

  it("cancels rename on Escape", async () => {
    const onRename = vi.fn();
    render(
      <SessionList sessions={baseSessions} variant="panel" onRename={onRename} />,
    );
    fireEvent.click(screen.getByTestId("session-list-rename-sess-12345678-abcdef"));
    const input = screen.getByTestId("session-list-rename-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Anything" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onRename).not.toHaveBeenCalled();
    expect(screen.queryByTestId("session-list-rename-input")).not.toBeInTheDocument();
  });

  it("does not call rename if value unchanged", async () => {
    const onRename = vi.fn();
    render(
      <SessionList sessions={baseSessions} variant="panel" onRename={onRename} />,
    );
    fireEvent.click(screen.getByTestId("session-list-rename-sess-12345678-abcdef"));
    const input = screen.getByTestId("session-list-rename-input");
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(onRename).not.toHaveBeenCalled();
    });
  });

  it("shows rename error if rename fails", async () => {
    const onRename = vi.fn().mockRejectedValue(new Error("boom"));
    render(
      <SessionList sessions={baseSessions} variant="panel" onRename={onRename} />,
    );
    fireEvent.click(screen.getByTestId("session-list-rename-sess-12345678-abcdef"));
    const input = screen.getByTestId("session-list-rename-input");
    fireEvent.change(input, { target: { value: "Updated" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/rename failed: boom/i);
    });
  });

  it("requires double-click to confirm delete", async () => {
    const onDelete = vi.fn().mockResolvedValue(undefined);
    render(
      <SessionList sessions={baseSessions} variant="panel" onDelete={onDelete} />,
    );
    const deleteBtn = screen.getByTestId("session-list-delete-sess-12345678-abcdef");
    fireEvent.click(deleteBtn);
    expect(onDelete).not.toHaveBeenCalled();
    expect(deleteBtn.className).toContain("session-list__item-delete--confirm");
    fireEvent.click(deleteBtn);
    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith("sess-12345678-abcdef");
    });
  });

  it("expanded variant has filter chips and sort headers", () => {
    render(<SessionList sessions={baseSessions} variant="expanded" />);
    expect(screen.getByTestId("session-list-filter-all")).toBeInTheDocument();
    expect(screen.getByTestId("session-list-filter-active")).toBeInTheDocument();
    expect(screen.getByTestId("session-list-filter-closed")).toBeInTheDocument();
    expect(screen.getByTestId("session-list-sort-started_at")).toBeInTheDocument();
    expect(screen.getByTestId("session-list-sort-agent_id")).toBeInTheDocument();
    expect(screen.getByTestId("session-list-sort-status")).toBeInTheDocument();
  });

  it("filter chips update filtered list", () => {
    render(<SessionList sessions={baseSessions} variant="expanded" />);
    fireEvent.click(screen.getByTestId("session-list-filter-closed"));
    expect(screen.queryByText(/Planning weekly tasks/i)).not.toBeInTheDocument();
    expect(screen.getByText(/Debugging memory leak/i)).toBeInTheDocument();
  });

  it("sort headers toggle direction when clicked twice", () => {
    render(<SessionList sessions={baseSessions} variant="expanded" />);
    const sortBtn = screen.getByTestId("session-list-sort-started_at");
    expect(sortBtn.textContent).toContain("↓");
    fireEvent.click(sortBtn);
    expect(sortBtn.textContent).toContain("↑");
  });
});
