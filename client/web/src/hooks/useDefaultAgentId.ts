// useDefaultAgentId — single source of truth for "which agent should
// the chat send messages to by default".
//
// Why this exists:
//   The daemon's `GET /v1/agents` returns the in-process default agent
//   with the id it actually accepts for session-create / message-send.
//   Previously the frontend hardcoded `"default"` in four places
//   (useChatTabs, ChatPanel ×2, AgentSelector). If the
//   daemon ever renames the in-process agent, refactors the default,
//   or runs in a multi-agent setup where the user wants a different
//   fallback, all five would silently send a stale id and the daemon
//   would reject with `agent not found`.
//
// What it does:
//   Reads `state.agents` from the StoreContext (populated at app mount
//   from `GET /v1/agents`, includes in_process / persistent / ephemeral
//   agents) and returns the id of the first in-process agent.
//
// Returns `null` when:
//   - the daemon hasn't reported any agents yet (initial load race)
//   - the daemon has no in-process agent (degenerate config)
//   - the daemon is unreachable
//
// Callers should fall back to a sentinel (empty string) when this
// returns null — that way the daemon returns a clear `agent_id
// required` error instead of `agent not found: <stale-id>`, which is
// harder to diagnose.

import { useStore } from "../store/StoreContext";

export function useDefaultAgentId(): string | null {
  const { state } = useStore();
  const firstInProcess = [...state.agents.values()].find(
    // The daemon returns `type: "in_process"` (not `kind`); the
    // StoreContext reducer preserves both fields on the Agent, so
    // check either.
    (a) => a.kind === "in_process" || a.type === "in_process",
  );
  return firstInProcess?.id ?? null;
}