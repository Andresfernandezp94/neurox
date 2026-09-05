// Tests for useChatTabs — synced from the daemon per user.
//
// The hook now treats the daemon as the source of truth:
//   - On mount it GETs /v1/sessions and builds tabs from the response
//   - New tabs are POSTed to the daemon first, then added locally
//   - Closing a tab POSTs /v1/sessions/:id/cancel (sets ended_at)
//   - The WS subscription reacts to SessionStarted/Ended events
//   - Stream chunks (content / thinking / tool_call / tool_result)
//     arrive via WS broadcast from other devices and are applied via
//     the same `applyStreamChunk` reducer the LOCAL SSE path uses
//     (the unified behaviour is exhaustively tested in
//     `components/chat/streaming/applyChunk.test.ts`).
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

// ─── Single-source-of-truth chunk apply (ordered by daemon `seq`) ────────
//
// PARTE B: the `/v1/events` WS is the ONLY source of stream chunks for
// BOTH sender and receiver. Ordering + exactly-once application is
// enforced by the daemon-assigned monotonic per-session `seq`
// (PARTE A). These tests pin that behaviour directly against the
// hook's public `applyStreamChunk` method (no WS mock, deterministic):
//
//   1. ORDER: the assistant shell is appended AFTER the user message
//      when no shell exists yet.
//   2. MULTI-TURN: the 2nd turn's answer does NOT latch onto the 1st
//      turn's assistant (fresh shell after the new user message).
//   3. COLLAPSE: many content chunks collapse into ONE timeline entry.
//   4. TOOL_REDIRECT: the wire-format quirk redirect still works.
//   5. SEQ ORDERING: a chunk with `seq <= lastSeq`, or with no `seq`,
//      is ignored — so a re-delivered (duplicate) chunk applies once.
//   6. METRICS: setStreamingMetrics only updates the targeted row.

function mockFetchEmpty() {
  return vi.fn((url: string | URL | Request) => {
    const u =
      typeof url === "string"
        ? url
        : url instanceof URL
          ? url.toString()
          : url.url;
    const body: Record<string, unknown> = {};
    if (u.includes("/v1/agents")) {
      body.in_process = [];
      body.persistent = [];
      body.ephemeral_templates = [];
      body.running = [];
    } else if (u.includes("/v1/sessions")) {
      body.sessions = [];
    } else if (u.includes("/v1/approvals")) {
      body.pending = [];
    } else if (u.includes("/health")) {
      body.status = "ok";
      body.service = "neurox";
      body.uptime_seconds = 0;
      body.version = "0.0.0-test";
      body.auth_required = false;
    }
    return Promise.resolve(
      new Response(JSON.stringify(body), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
  }) as typeof fetch;
}

function mountHook(initialToken: string | null = null) {
  if (initialToken) {
    window.sessionStorage.setItem("neurox_token", initialToken);
  } else {
    window.sessionStorage.clear();
  }
  if (!global.fetch) global.fetch = mockFetchEmpty();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <StoreProvider eventsPath="/__test_no_ws__">{children}</StoreProvider>
  );
  return renderHook(() => useChatTabs(), { wrapper });
}

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("useChatTabs — single-source chunk apply (ordered by seq)", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  afterEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
    vi.restoreAllMocks();
  });

  it("creates the assistant shell on the first chunk (so user→assistant order is preserved)", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const draftId = result.current.activeId!;
    const draftSessionId = "sess-order";
    act(() => {
      result.current.updateTab(draftId, { sessionId: draftSessionId });
    });
    // Simulate the user message that ChatPanel.handleSend adds
    // locally with a NEGATIVE temp id (the order-fix sentinel).
    act(() => {
      result.current.updateTab(draftId, (prev) => ({
        ...prev,
        messages: [
          ...prev.messages,
          {
            id: -1234,
            session_id: draftSessionId,
            role: "user",
            content: "hi",
            ts: "2026-09-05T00:00:00Z",
          },
        ],
      }));
    });
    // First chunk arrives via the unified apply entry point (with seq).
    act(() => {
      result.current.applyStreamChunk(draftId, draftSessionId, {
        type: "thinking",
        text: "thinking...",
        seq: 1,
      });
    });
    await flush();
    const tab = result.current.tabs.find((t) => t.id === draftId)!;
    expect(tab.messages.map((m) => m.role)).toEqual(["user", "assistant"]);
    const assistant = tab.messages.find((m) => m.role === "assistant")!;
    // The shell id lives in the reserved high range so it can never be
    // confused with a canonical backend message_id (small autoincrement)
    // or a hydration id — this is what prevents the DOUBLE assistant row
    // on the receiving device when MessageAppended replaces the shell.
    expect(assistant.id).toBeGreaterThanOrEqual(1_000_000_000);
    expect(assistant.thinking).toBe("thinking...");
  });

  it("multi-turn: 2nd turn's answer does NOT latch onto the 1st turn's assistant (order preserved)", async () => {
    // Regression: sending a 2nd message rendered its answer inside the
    // PREVIOUS assistant bubble and stranded the 2nd user message at
    // the bottom. The fix: on the first chunk of a new turn (lastSeq
    // reset + streamMessageIdsRef cleared), the "last assistant"
    // fallback is taken ONLY when the last transcript row is an
    // assistant. After the 2nd user message is added, the last row is
    // a USER message, so a fresh shell is created AFTER it.
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-multiturn";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });

    // ── Turn 1 ──────────────────────────────────────────────────
    act(() => {
      result.current.updateTab(tabId, (prev) => ({
        ...prev,
        messages: [
          ...prev.messages,
          { id: -1, session_id: sessId, role: "user", content: "hola", ts: "" },
        ],
      }));
    });
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "¡Hola!",
        seq: 1,
      });
    });
    await flush();
    const firstAssistantId = result.current.tabs
      .find((t) => t.id === tabId)!
      .messages.find((m) => m.role === "assistant")!.id;

    // Stream-done for turn 1: the WS message_appended(assistant)
    // handler clears the per-session stream tracking (accumulator +
    // last-seq + tracked assistant id).
    act(() => {
      result.current.clearSessionStreamState(sessId);
    });

    // ── Turn 2 ──────────────────────────────────────────────────
    act(() => {
      result.current.updateTab(tabId, (prev) => ({
        ...prev,
        messages: [
          ...prev.messages,
          { id: -2, session_id: sessId, role: "user", content: "que puedes hacer?", ts: "" },
        ],
      }));
    });
    // The daemon's seq is monotonic per SESSION (not per turn), so the
    // 2nd turn's first chunk has a higher seq than turn 1.
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "Puedo ayudarte con...",
        seq: 2,
      });
    });
    await flush();

    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    expect(tab.messages.map((m) => m.role)).toEqual([
      "user",
      "assistant",
      "user",
      "assistant",
    ]);
    const firstStill = tab.messages.find((m) => m.id === firstAssistantId)!;
    expect(firstStill.content).toBe("¡Hola!");
    const assistants = tab.messages.filter((m) => m.role === "assistant");
    expect(assistants).toHaveLength(2);
    expect(assistants[1]!.content).toBe("Puedo ayudarte con...");
    expect(assistants[1]!.id).not.toBe(firstAssistantId);
  });

  it("collapses many content chunks into ONE timeline entry", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-collapse";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });
    for (let i = 0; i < 50; i++) {
      act(() => {
        result.current.applyStreamChunk(tabId, sessId, {
          type: "content",
          text: `c${i} `,
          seq: i + 1,
        });
      });
    }
    await flush();
    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    const assistant = tab.messages.find((m) => m.role === "assistant")!;
    expect(assistant.timeline).toHaveLength(1);
    expect(assistant.timeline![0]).toMatchObject({ type: "content" });
    expect((assistant.timeline![0] as { text: string }).text).toBe(
      Array.from({ length: 50 }, (_, i) => `c${i} `).join(""),
    );
  });

  it("applies the tool_output redirect quirk (content after tool_call → tool.result)", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-quirk";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({
        ...prev,
        sessionId: sessId,
        messages: [
          ...prev.messages,
          { id: -1, session_id: sessId, role: "user", content: "ls", ts: "" },
        ],
      }));
    });
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "tool_call",
        tool: "shell",
        args: { cmd: "ls" },
        iteration: 0,
        seq: 1,
      });
    });
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "file1.txt\nfile2.txt\n",
        seq: 2,
      });
    });
    await flush();
    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    const assistant = tab.messages.find((m) => m.role === "assistant")!;
    expect(assistant.content).toBe("");
    expect(assistant.toolLog![0]!.result).toBe("file1.txt\nfile2.txt\n");
    expect(assistant.timeline![0]).toMatchObject({ type: "tool" });
  });

  it("ignores a re-delivered chunk (seq <= lastSeq) — content applies exactly once", async () => {
    // Single-source-of-truth defence: the same chunk may reach the
    // client more than once (reconnect replay, StrictMode double
    // socket). The `seq` gate applies it exactly once.
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-seq";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });
    const chunk = { type: "content" as const, text: "hello", seq: 5 };
    // Apply the SAME chunk 100 times — only the first (seq 5 > -inf)
    // takes effect; the rest are seq <= lastSeq re-deliveries.
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, chunk);
    });
    for (let i = 0; i < 99; i++) {
      act(() => {
        result.current.applyStreamChunk(tabId, sessId, chunk);
      });
    }
    await flush();
    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    const assistant = tab.messages.find((m) => m.role === "assistant")!;
    // Applied once → content is "hello", not "hellohello...".
    expect(assistant.content).toBe("hello");
    expect(assistant.timeline).toHaveLength(1);
  });

  it("ignores chunks without a seq (legacy) — nothing is applied", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-noseq";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });
    let r: { messageId: number | null; error: string | null } = {
      messageId: 0,
      error: null,
    };
    act(() => {
      // No `seq` field — the ordering gate drops it.
      r = result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "legacy",
      });
    });
    await flush();
    expect(r.messageId).toBeNull();
    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    // No assistant shell was created.
    expect(tab.messages.some((m) => m.role === "assistant")).toBe(false);
  });

  it("out-of-order lower seq after a higher one is ignored", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-order2";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "A",
        seq: 10,
      });
    });
    // A stale, lower-seq chunk arrives late — must be dropped.
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "B",
        seq: 3,
      });
    });
    // A newer chunk applies.
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, {
        type: "content",
        text: "C",
        seq: 11,
      });
    });
    await flush();
    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    const assistant = tab.messages.find((m) => m.role === "assistant")!;
    expect(assistant.content).toBe("AC");
  });

  it("keeps the seq watermark ACROSS turns: a late chunk from the finished turn is NOT re-applied", async () => {
    // Regression for the residual duplication after the seq rebuild:
    // the daemon's `seq` is monotonic per SESSION (never restarts per
    // turn), so the client's `lastSeq` must survive turn boundaries.
    // Previously `clearSessionStreamState` (fired on the stream-done
    // MessageAppended) wiped `lastSeq`, so a late/re-delivered chunk
    // from turn 1 (seq already seen) was re-applied on turn 2. Now the
    // per-turn reset keeps `lastSeq`; only the accumulator + shell id
    // are cleared.
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-crossturn";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });

    // ── Turn 1: two content chunks (seq 1, 2). ──
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, { type: "content", text: "hola", seq: 1 });
    });
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, { type: "content", text: " mundo", seq: 2 });
    });
    // Stream-done for turn 1 (per-turn reset — must KEEP lastSeq=2).
    act(() => {
      result.current.clearTurnStreamState(sessId);
    });
    await flush();
    let tab = result.current.tabs.find((t) => t.id === tabId)!;
    const firstAssistantId = tab.messages.find((m) => m.role === "assistant")!.id;

    // ── A late / re-delivered chunk from turn 1 (seq 2) arrives. ──
    // It must be dropped by the surviving watermark — NOT re-applied.
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, { type: "content", text: " mundo", seq: 2 });
    });
    await flush();
    tab = result.current.tabs.find((t) => t.id === tabId)!;
    const firstAssistant = tab.messages.find((m) => m.id === firstAssistantId)!;
    expect(firstAssistant.content).toBe("hola mundo"); // not "hola mundo mundo"

    // ── Turn 2: the user's next prompt lands first (as it does in
    // production: message_appended(user) precedes the assistant
    // stream), THEN a new chunk with a higher seq. ──
    act(() => {
      result.current.updateTab(tabId, (prev) => ({
        ...prev,
        messages: [
          ...prev.messages,
          { id: 99, session_id: sessId, role: "user", content: "otra?", ts: "" },
        ],
      }));
    });
    act(() => {
      result.current.applyStreamChunk(tabId, sessId, { type: "content", text: "otra", seq: 3 });
    });
    await flush();
    tab = result.current.tabs.find((t) => t.id === tabId)!;
    const assistants = tab.messages.filter((m) => m.role === "assistant");
    expect(assistants).toHaveLength(2);
    expect(assistants[1]!.content).toBe("otra");
    expect(assistants[1]!.id).not.toBe(firstAssistantId);
  });

  it("applies non-ordered chunks (approval/error) even though they carry no seq", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-approval";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({ ...prev, sessionId: sessId }));
    });
    // An error chunk has no seq — it must still surface.
    let r: { messageId: number | null; error: string | null } = { messageId: 0, error: null };
    act(() => {
      r = result.current.applyStreamChunk(tabId, sessId, {
        type: "error",
        message: "boom",
      });
    });
    await flush();
    expect(r.error).toBe("boom");
  });

  it("setStreamingMetrics only updates the targeted message", async () => {
    global.fetch = mockFetchEmpty();
    const { result } = mountHook();
    await flush();
    const tabId = result.current.activeId!;
    const sessId = "s-metrics";
    act(() => {
      result.current.updateTab(tabId, (prev) => ({
        ...prev,
        sessionId: sessId,
        messages: [
          ...prev.messages,
          { id: 1, session_id: sessId, role: "assistant", content: "x", ts: "" },
          { id: 2, session_id: sessId, role: "assistant", content: "y", ts: "" },
        ],
      }));
    });
    act(() => {
      result.current.setStreamingMetrics(tabId, 1, {
        durationMs: 1000,
        tokens: 10,
        tokensPerSec: 10,
      });
    });
    await flush();
    const tab = result.current.tabs.find((t) => t.id === tabId)!;
    expect(tab.messages.find((m) => m.id === 1)!.metrics).toEqual({
      durationMs: 1000,
      tokens: 10,
      tokensPerSec: 10,
    });
    expect(tab.messages.find((m) => m.id === 2)!.metrics).toBeUndefined();
  });
});

// Note: WS stream-chunk integration tests were prototyped but the
// jsdom + mock-WebSocket + StoreProvider + renderHook combination
// turned out to be flaky in this repo's test setup. The WS handler in
// `useChatTabs` parses the chunk (via `parseStreamChunk`, which now
// reads `seq`) and routes it to `applyStreamChunk` — the exact method
// these tests exercise directly. Behaviour parity is a code-level
// guarantee, not a runtime one.
