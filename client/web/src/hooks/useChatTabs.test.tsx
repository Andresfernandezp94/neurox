// Tests for useChatTabs — synced from the daemon per user.
//
// The hook now treats the daemon as the source of truth:
//   - On mount it GETs /v1/sessions and builds tabs from the response
//   - New tabs are POSTed to the daemon first, then added locally
//   - Closing a tab POSTs /v1/sessions/:id/cancel (sets ended_at)
//   - The WS subscription reacts to SessionStarted/Ended events
//
// `withSessions` mocks /v1/sessions to return a configurable list
// and /v1/agents to a stub (StoreProvider's snapshot fetch).

import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChatTabs } from "./useChatTabs";
import { StoreProvider } from "../store/StoreContext";

const TEST_AGENT_ID = "default";

interface SessionRow {
  session_id: string;
  agent_id: string;
  started_at: string;
  ended_at: string | null;
  summary: string | null;
  provider_id?: string | null;
  model?: string | null;
}

describe("useChatTabs — daemon-synced tabs", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("hydrates tabs from /v1/sessions on mount", async () => {
    const sessions: SessionRow[] = [
      {
        session_id: "sess-a",
        agent_id: TEST_AGENT_ID,
        started_at: "2026-09-05T00:00:00Z",
        ended_at: null,
        summary: "first chat",
      },
      {
        session_id: "sess-b",
        agent_id: TEST_AGENT_ID,
        started_at: "2026-09-05T00:01:00Z",
        ended_at: null,
        summary: null,
      },
    ];
    const wrapper = ({ children }: { children: ReactNode }) => (
      <StoreProvider eventsPath="/__test_no_ws__">{children}</StoreProvider>
    );
    global.fetch = vi.fn((url: string | URL | Request) => {
      const u = typeof url === "string" ? url : url instanceof URL ? url.toString() : url.url;
      if (u.includes("/v1/agents")) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              in_process: [{ id: TEST_AGENT_ID, kind: "in_process", status: "ready" }],
              persistent: [],
              ephemeral_templates: [],
              running: [],
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      if (u.includes("/v1/sessions")) {
        return Promise.resolve(
          new Response(JSON.stringify({ sessions }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }),
        );
      }
      if (u.includes("/v1/approvals")) {
        return Promise.resolve(
          new Response(JSON.stringify({ pending: [] }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }),
        );
      }
      if (u.includes("/health")) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              status: "ok",
              service: "neurox",
              uptime_seconds: 0,
              version: "0.0.0-test",
              auth_required: false,
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as typeof fetch;

    const { result } = renderHook(() => useChatTabs(), { wrapper });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.tabs).toHaveLength(2);
    expect(result.current.tabs[0]!.sessionId).toBe("sess-a");
    expect(result.current.tabs[0]!.title).toBe("first chat");
    expect(result.current.tabs[1]!.sessionId).toBe("sess-b");
    expect(result.current.tabs[1]!.title).toBe("Chat 2");
  });

  it("auto-creates a draft tab when the daemon returns no sessions", async () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <StoreProvider eventsPath="/__test_no_ws__">{children}</StoreProvider>
    );
    global.fetch = vi.fn((url: string | URL | Request) => {
      const u = typeof url === "string" ? url : url instanceof URL ? url.toString() : url.url;
      if (u.includes("/v1/agents")) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              in_process: [{ id: TEST_AGENT_ID, kind: "in_process", status: "ready" }],
              persistent: [],
              ephemeral_templates: [],
              running: [],
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      if (u.includes("/v1/sessions")) {
        return Promise.resolve(
          new Response(JSON.stringify({ sessions: [] }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }),
        );
      }
      if (u.includes("/v1/approvals")) {
        return Promise.resolve(
          new Response(JSON.stringify({ pending: [] }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }),
        );
      }
      if (u.includes("/health")) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              status: "ok",
              service: "neurox",
              uptime_seconds: 0,
              version: "0.0.0-test",
              auth_required: false,
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as typeof fetch;

    const { result } = renderHook(() => useChatTabs(), { wrapper });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.tabs[0]!.title).toBe("Chat 1");
    expect(result.current.tabs[0]!.sessionId).toBeNull();
    expect(result.current.activeId).toBe(result.current.tabs[0]!.id);
  });
});
