// EP-0026-01 R3.1: tests del auto-create de tab al montar.
//
// El hook debe crear una tab "Chat 1" cuando localStorage está
// vacío (primera vez que el usuario abre el chat), y NO debe
// crear una si ya hay tabs en localStorage.
//
// `useChatTabs` ahora delega el `sessionAgent` por defecto a
// `useDefaultAgentId()` (lee de `state.agents`), por lo que los
// tests wrappean con `StoreProvider` y simulan un agente
// in_process con id `default` (el que el daemon reporta
// actualmente via `/v1/agents`).

import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChatTabs } from "./useChatTabs";
import { StoreProvider } from "../store/StoreContext";

const STORAGE_KEY = "chat-tabs-v1";
const ACTIVE_KEY = "chat-active-tab-v1";
const TEST_AGENT_ID = "default";

/**
 * Minimal StoreProvider wrapper that pre-populates the agents map
 * with one in_process entry (the test agent id). We mock
 * `/v1/agents` fetch so StoreProvider doesn't try to hit the real
 * daemon during tests.
 */
function withAgents({ children }: { children: ReactNode }) {
  // Patch fetch to return the test agent without hitting the daemon.
  const originalFetch = global.fetch;
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
        new Response(
          JSON.stringify({ sessions: [], specs: [], details: [], running: [] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }
    if (u.includes("/v1/approvals")) {
      // Daemon returns `{pending: Approval[]}` (NOT `approvals`).
      // StoreContext.tsx:303 reads `.pending` — match the real shape.
      return Promise.resolve(
        new Response(
          JSON.stringify({ pending: [] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
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
    return originalFetch(url as Request);
  }) as typeof fetch;

  return (
    <StoreProvider eventsPath="/__test_no_ws__{Math.random()}">{children}</StoreProvider>
  );
}

describe("useChatTabs — auto-create on mount (EP-0026-01 R1)", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("creates a default tab when localStorage is empty", async () => {
    const { result } = renderHook(() => useChatTabs(), { wrapper: withAgents });

    // Wait for the auto-create effect to flush.
    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.tabs).toHaveLength(1);
    const tab = result.current.tabs[0]!;
    expect(tab.title).toBe("Chat 1");
    expect(tab.sessionId).toBeNull();
    expect(tab.sessionModel).toBeNull();
    // sessionAgent is the empty sentinel — the consumer (ChatPanel)
    // resolves the real daemon agent id at message-send time via
    // useDefaultAgentId(). This avoids a race where the tab is
    // created before the StoreProvider's /v1/agents fetch completes.
    expect(tab.sessionAgent).toBe("");
    expect(tab.messages).toEqual([]);
    expect(typeof tab.id).toBe("string");
    expect(tab.id.length).toBeGreaterThan(0);
  });

  it("sets activeId to the new tab when localStorage is empty", async () => {
    const { result } = renderHook(() => useChatTabs(), { wrapper: withAgents });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.activeId).toBe(result.current.tabs[0]!.id);
    expect(result.current.activeTab?.id).toBe(result.current.activeId);
  });

  it("does NOT create a tab when localStorage already has tabs", async () => {
    const existing = [
      {
        id: "tab-existing",
        title: "Pre-existing chat",
        sessionId: "sess-1",
        sessionModel: null,
        // Legacy id — normalizeAgentId maps it to the empty sentinel
        // so the runtime resolver (useDefaultAgentId) fills it in.
        sessionAgent: "default",
        messages: [],
        createdAt: 1700000000000,
      },
    ];
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(existing));
    window.localStorage.setItem(ACTIVE_KEY, "tab-existing");

    const { result } = renderHook(() => useChatTabs(), { wrapper: withAgents });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.tabs[0]!.id).toBe("tab-existing");
    expect(result.current.activeId).toBe("tab-existing");
    // Legacy "default" id was migrated to empty string; the
    // consumer (ChatPanel) will resolve it via useDefaultAgentId()
    // at message-send time.
    expect(result.current.tabs[0]!.sessionAgent).toBe("");
  });

  it("persists the auto-created tab to localStorage", async () => {
    const { result } = renderHook(() => useChatTabs(), { wrapper: withAgents });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.tabs).toHaveLength(1);

    // The persist effect runs after the state settles; wait one
    // microtask for the effect to flush.
    await act(async () => {
      await Promise.resolve();
    });

    const stored = window.localStorage.getItem(STORAGE_KEY);
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored!) as unknown[];
    expect(parsed).toHaveLength(1);
  });
});