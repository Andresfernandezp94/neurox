// useDefaultAgentId — the runtime resolver for "which agent id
// should the chat use by default". Reads `state.agents` from the
// StoreContext and returns the first in_process agent's id.
//
// Critical: the daemon returns agents with a `type` field (not
// `kind` — see `/v1/agents` response shape). The hook must check
// both so it works against the real daemon (which uses `type`) and
// any frontend mirror that adds `kind`.

import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDefaultAgentId } from "./useDefaultAgentId";

// Mock useStore so each test can plug in its own agents map.
const mockUseStore = vi.fn();
vi.mock("../store/StoreContext", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../store/StoreContext")>();
  return {
    ...actual,
    useStore: () => mockUseStore(),
  };
});

interface AgentLike {
  id: string;
  kind?: string;
  type?: string;
  status?: string;
}

function setAgents(agents: AgentLike[]) {
  const map = new Map<string, AgentLike>();
  for (const a of agents) map.set(a.id, a);
  mockUseStore.mockReturnValue({ state: { agents: map } });
}

describe("useDefaultAgentId", () => {
  beforeEach(() => {
    mockUseStore.mockReset();
  });

  it("returns the id of the first in_process agent (daemon shape: `type`)", () => {
    // The actual daemon `/v1/agents` returns `{id, type, status}`
    // — no `kind` field. The hook must accept this.
    setAgents([
      { id: "default", type: "in_process", status: "ready" },
    ]);
    const { result } = renderHook(() => useDefaultAgentId());
    expect(result.current).toBe("default");
  });

  it("also accepts `kind` (frontend mirror shape)", () => {
    // The StoreContext sometimes adds `kind` from the merged agent
    // shape (e.g. when an agent arrives via WS event).
    setAgents([
      { id: "default", kind: "in_process", status: "ready" },
    ]);
    const { result } = renderHook(() => useDefaultAgentId());
    expect(result.current).toBe("default");
  });

  it("returns null when no in_process agent is loaded", () => {
    setAgents([{ id: "some_persistent", kind: "persistent" }]);
    const { result } = renderHook(() => useDefaultAgentId());
    expect(result.current).toBeNull();
  });

  it("returns null when no agents are loaded yet (daemon unreachable / initial load)", () => {
    setAgents([]);
    const { result } = renderHook(() => useDefaultAgentId());
    expect(result.current).toBeNull();
  });
});