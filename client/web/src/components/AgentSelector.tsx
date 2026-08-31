// AgentSelector — EP-0024 / EP-2026-08-15.
// Dropdown single-select del agent de la sesión. La barra muestra
// solo el agent activo; click abre la lista. El usuario elige y se
// persiste (no se recrea la sesión).
//
// EP-2026-08-15: el daemon (a través de /v1/agents, fuente única)
// lista los agentes disponibles. Por debajo de los items hay
// controles inline para crear uno nuevo (POST /v1/agents/in_process)
// y borrar los custom (DELETE …) sin reiniciar el daemon.

import { useEffect, useState, useCallback, useRef } from "react";
import { apiGet, apiPost, apiDelete } from "../api/client";
import { IconRobot } from "../shared/components/Icons";

export interface AgentSelectorProps {
  currentAgent?: string | null;
  onChange?: (agentId: string) => void;
  disabled?: boolean;
}

interface AgentItem {
  id: string;
  status?: string;
  kind?: string;
}

// Sentinel used when /v1/agents is unreachable AND no currentAgent
// prop is set. Renders an empty id — better than a stale hardcoded
// string that the daemon would reject with `agent not found`.
// Single source of truth lives in `useDefaultAgentId()`; this
// component still fetches `/v1/agents` directly to populate the
// dropdown.
const NO_AGENT_SENTINEL = "";

// Parse the daemon's `/v1/agents` response. The body is a *shaped*
// object, not a flat list — see `list_agents` in
// `daemon/core/src/router/http.rs`:
//
//   { persistent: [...], ephemeral_templates: [...],
//     running: [...], in_process: [{ type: "in_process", id, status }] }
//
// The previous parser looked at `data.agents` (which doesn't exist),
// so the dropdown always ended up empty and the only item shown was
// the FALLBACK_AGENT. The AgentsPanel reads the same endpoint via the
// store and renders the merged list correctly — we mirror that logic
// here so the chat dropdown matches the panel.
function parseAgents(data: unknown): AgentItem[] {
  if (!data || typeof data !== "object") return [];
  const o = data as Record<string, unknown>;
  const result: AgentItem[] = [];

  const pushAll = (section: unknown, kind: string, status?: string) => {
    if (!Array.isArray(section)) return;
    for (const raw of section) {
      if (raw && typeof raw === "object") {
        const r = raw as Record<string, unknown>;
        const id = typeof r.id === "string" ? r.id : "";
        if (!id) continue;
        const itemStatus =
          typeof r.status === "string" ? r.status : status;
        // EP-2026-08-15: per-item `kind` from the daemon wins over
        // the section default. This is how the daemon signals
        // `session-isolated` for session_agents entries vs. plain
        // in_process.
        const itemKind =
          typeof r.kind === "string" ? r.kind : kind;
        result.push({ id, status: itemStatus, kind: itemKind });
      }
    }
  };

  // persistent + ephemeral_templates don't carry a runtime status — they're
  // configured specs, not running processes. We mark them as "configured".
  pushAll(o.persistent, "persistent", "configured");
  pushAll(o.ephemeral_templates, "ephemeral", "configured");
  pushAll(o.running, "running");
  // EP-2026-08-15: in_process entries now carry a per-item `kind`
  // from the daemon (e.g. "session-isolated" for session_agents
  // specs). We surface that kind so the UI can style synthetic
  // session agents (admin / user) distinctly.
  pushAll(o.in_process, "in_process");

  // EP-2026-08-15: NO synthetic injection. The daemon is the single
// source of truth — admin / user / runtime-created all come from
// `/v1/agents`. If the daemon returns nothing (offline / older build
// without session_agents configured), we render an empty list and
// the empty-state hint guides the user to configure the daemon.
  return result;
}

export function AgentSelector({ currentAgent, onChange, disabled }: AgentSelectorProps) {
  const [agents, setAgents] = useState<AgentItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [userSelection, setUserSelection] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  // EP-2026-08-15: inline-create form state.
  const [showCreate, setShowCreate] = useState(false);
  const [newId, setNewId] = useState("");
  const [newIdentityDir, setNewIdentityDir] = useState("");
  const [creating, setCreating] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // EP-2026-08-15: POST /v1/agents/in_process with the inline form.
  // Builds the spec on the daemon, which also materializes the
  // identity_dir template (system-prompt.md / _always-on.md /
  // facts.yaml / skills/).
  const handleCreate = useCallback(async () => {
    const id = newId.trim();
    const identityDir = newIdentityDir.trim();
    if (!id || !identityDir) {
      setError("agent id and identity dir are required");
      return;
    }
    setCreating(true);
    setError(null);
    try {
      await apiPost("/v1/agents/in_process", {
        id,
        identity_dir: identityDir,
      });
      setNewId("");
      setNewIdentityDir("");
      setShowCreate(false);
      // EP-2026-08-15: refresh the list inline (refreshAgents is
      // declared below). The new agent shows up immediately.
      try {
        const data = await apiGet<unknown>("/v1/agents");
        setAgents(parseAgents(data));
      } catch {
        setAgents([]);
      }
    } catch (err) {
      setError(`create failed: ${(err as Error).message}`);
    } finally {
      setCreating(false);
    }
  }, [newId, newIdentityDir]);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await apiGet<unknown>("/v1/agents");
        const list = parseAgents(data);
        setAgents(list);
      } catch {
        setAgents([]);   // EP-2026-08-15: empty list (daemon offline / older build)
      } finally {
        setLoading(false);
      }
    };
    void load();
  }, []);

  // EP-0024: re-fetch cuando se abre el dropdown.
  const refreshAgents = useCallback(async () => {
    try {
      const data = await apiGet<unknown>("/v1/agents");
      setAgents(parseAgents(data));
    } catch {
      // Silencioso.
    }
  }, []);

  useEffect(() => {
    if (open) {
      void refreshAgents();
    }
  }, [open, refreshAgents]);

  // EP-2026-08-15 (switch-agent live fix): `userSelection` is local
  // state that tracks the user's most-recent click. It overrides
  // `currentAgent` until the parent updates sessionAgent. Without
  // this reset, switching tabs keeps the old `userSelection` from
  // the previous tab → dropdown shows a checkmark on the wrong agent
  // (UI desync from the actual tab session). Clearing on every
  // `currentAgent` change keeps the dropdown honest.
  useEffect(() => {
    setUserSelection(null);
  }, [currentAgent]);

  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, [open]);

// Pick the agent to display: explicit user choice, then the
// caller-supplied current agent, then the first synthetic
// session-isolated agent (admin / user) injected locally, then the
// first daemon-reported in-process agent, then an empty sentinel.
const firstSessionIsolated = agents.find((a) => a.kind === "session-isolated")?.id;
const firstInProcess = agents.find((a) => a.kind === "in_process")?.id;
const selected =
  userSelection ?? currentAgent ?? firstSessionIsolated ?? firstInProcess ?? NO_AGENT_SENTINEL;

  const handleSelect = useCallback(
    async (id: string) => {
      if (id === selected) return;
      setUserSelection(id);
      setError(null);
      try {
        onChange?.(id);
        setOpen(false);
      } catch (err) {
        setUserSelection(null);
        setError((err as Error).message);
      }
    },
    [selected, onChange],
  );

  return (
    <div className="agent-selector" ref={wrapperRef}>
      <button
        type="button"
        className={
          "chat__bar-actions__btn" + (open ? " chat__bar-actions__btn--active" : "")
        }
        onClick={() => setOpen((o) => !o)}
        disabled={disabled || loading}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Agent: ${selected}`}
        title={selected || "Agent"}
        data-testid="chat-agent-toggle"
      >
        <IconRobot />
      </button>

      {open && (
        <div className="agent-selector__dropdown" role="listbox" data-testid="agent-selector-dropdown">
          <div className="agent-selector__search">
            <span className="agent-selector__search-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="11" cy="11" r="7" />
                <path d="M21 21l-4.3-4.3" />
              </svg>
            </span>
            <input
              type="text"
              className="agent-selector__search-input"
              placeholder="search…"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              autoFocus
              data-testid="agent-selector-search"
            />
            <button
              type="button"
              className="agent-selector__refresh"
              title="Refresh agents"
              aria-label="Refresh agents"
              onClick={() => void refreshAgents()}
              data-testid="agent-selector-refresh"
            >
              ↻
            </button>
          </div>
          {agents
            .filter((a) => {
              if (!filter) return true;
              const q = filter.toLowerCase();
              return a.id.toLowerCase().includes(q) ||
                (a.kind?.toLowerCase().includes(q) ?? false);
            })
            .map((a) => {
              const isActive = selected === a.id;
              const isSessionIsolated = a.kind === "session-isolated";
              return (
                <div
                  key={a.id}
                  role="option"
                  aria-selected={isActive}
                  className={`agent-selector__item${isActive ? " agent-selector__item--active" : ""}${isSessionIsolated ? " agent-selector__item--session" : ""}`}
                  data-testid={`agent-selector-item-${a.id}`}
                  onClick={() => void handleSelect(a.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      void handleSelect(a.id);
                    }
                  }}
                  role-button="true"
                  tabIndex={0}
                >
                  <span
                    className="agent-selector__name"
                    onClick={() => void handleSelect(a.id)}
                    role="button"
                    tabIndex={-1}
                  >
                    {a.id}
                  </span>
                  {a.kind && <span className="agent-selector__kind">{a.kind}</span>}
                  {isActive && <span className="agent-selector__check">✓</span>}
                  {/* EP-2026-08-15: delete button for every agent (admin/user
                   * included — admin/user are now created via the daemon
                   * config too, so it's safe to delete them if not used).
                   * Stops click from bubbling to the select. */}
                  <button
                    type="button"
                    className="agent-selector__delete"
                    title={`Delete agent ${a.id}`}
                    aria-label={`Delete agent ${a.id}`}
                    data-testid={`agent-selector-delete-${a.id}`}
                    onClick={async (e) => {
                      e.stopPropagation();
                      try {
                        await apiDelete(`/v1/agents/in_process/${encodeURIComponent(a.id)}`);
                        await refreshAgents();
                      } catch (err) {
                        setError(`could not delete ${a.id}: ${(err as Error).message}`);
                      }
                    }}
                  >
                    ×
                  </button>
                </div>
              );
            })}
          {agents.length === 0 && (
            <div className="agent-selector__empty muted text-xs">
              no agents configured
              <div className="agent-selector__empty-hint text-xs">
                Configure <code>session_agents</code> in the daemon, or add one below.
              </div>
            </div>
          )}
          {agents.length > 0 && filter && !agents.some((a) => a.id.toLowerCase().includes(filter.toLowerCase())) && (
            <div className="agent-selector__empty muted text-xs">no matches</div>
          )}
          {/* EP-2026-08-15: footer with "+" button to create a new agent
           * inline. Click expands an inline form. Sends
           * POST /v1/agents/in_process and refreshes the list. */}
          <div className="agent-selector__footer">
            {!showCreate ? (
              <button
                type="button"
                className="agent-selector__add"
                title="Create a new agent"
                aria-label="Create a new agent"
                data-testid="agent-selector-add"
                onClick={() => setShowCreate(true)}
              >
                + New agent
              </button>
            ) : (
              <form
                className="agent-selector__form"
                onSubmit={async (e) => {
                  e.preventDefault();
                  await handleCreate();
                }}
              >
                <input
                  type="text"
                  className="agent-selector__form-input"
                  placeholder="agent id (e.g. developer)"
                  value={newId}
                  onChange={(e) => setNewId(e.target.value)}
                  autoFocus
                  data-testid="agent-selector-form-id"
                />
                <input
                  type="text"
                  className="agent-selector__form-input"
                  placeholder="identity dir (absolute path)"
                  value={newIdentityDir}
                  onChange={(e) => setNewIdentityDir(e.target.value)}
                  data-testid="agent-selector-form-identitydir"
                />
                <div className="agent-selector__form-actions">
                  <button
                    type="submit"
                    className="agent-selector__form-submit"
                    disabled={creating}
                    data-testid="agent-selector-form-submit"
                  >
                    {creating ? "Creating…" : "Create"}
                  </button>
                  <button
                    type="button"
                    className="agent-selector__form-cancel"
                    onClick={() => {
                      setShowCreate(false);
                      setNewId("");
                      setNewIdentityDir("");
                    }}
                  >
                    Cancel
                  </button>
                </div>
              </form>
            )}
          </div>
        </div>
      )}

      {error && <div className="agent-selector__error">{error}</div>}
    </div>
  );
}