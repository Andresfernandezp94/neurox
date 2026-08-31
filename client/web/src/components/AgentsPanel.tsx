// Panel de Agentes — consume el store global. EP-0003-04.
// EP-0024: tabs internas — "Agents" (default), "Skills" y "Tools".
// Cada agente se muestra como card con toggle switch on/off
// (active = running, inactivo = stopped).

import { useCallback, useEffect, useState } from "react";
import { useStore } from "../store/StoreContext";
import { startAgent, stopAgent } from "../api/agents";
import { apiGet } from "../api/client";
import { Stack } from "../shared/components/molecules/Stack";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { IconDefaultAgent } from "../shared/components/Icons";
import { ToolsExplorerTab } from "./ToolsExplorerTab";

export function AgentsPanel() {
  const { state, dispatch, snapshot } = useStore();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"agents" | "skills" | "tools">("agents");

  const agents = Array.from(state.agents.values());

  const handleToggle = useCallback(
    async (id: string, currentStatus: string | undefined) => {
      const isActive = currentStatus === "running" || currentStatus === "starting";
      setBusy(id);
      setError(null);
      try {
        if (isActive) {
          dispatch({ type: "AGENT_LOCAL_UPDATE", id, patch: { status: "stopping" } });
          await stopAgent(id);
        } else {
          dispatch({ type: "AGENT_LOCAL_UPDATE", id, patch: { status: "starting" } });
          await startAgent(id);
        }
        await snapshot();
      } catch (e) {
        setError((e as Error).message);
        await snapshot();
      } finally {
        setBusy(null);
      }
    },
    [dispatch, snapshot],
  );

  return (
    <Stack gap="md">
      <div className="agents-panel__tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "agents"}
          data-testid="agents-tab-agents"
          className={`agents-panel__tab ${tab === "agents" ? "active" : ""}`}
          onClick={() => setTab("agents")}
        >
          Agents
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "skills"}
          data-testid="agents-tab-skills"
          className={`agents-panel__tab ${tab === "skills" ? "active" : ""}`}
          onClick={() => setTab("skills")}
        >
          Skills
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "tools"}
          data-testid="agents-tab-tools"
          className={`agents-panel__tab ${tab === "tools" ? "active" : ""}`}
          onClick={() => setTab("tools")}
        >
          Tools
        </button>
      </div>

      {tab === "agents" && (
        <>
          {error && <ErrorBanner>{error}</ErrorBanner>}

          {!state.loaded.agents && agents.length === 0 && (
            <div className="loading">Loading…</div>
          )}

          <div className="agents-list" role="list" data-testid="agents-list">
            {agents.map((a) => {
              const isActive = a.status === "running" || a.status === "starting";
              return (
                <div
                  key={a.id}
                  className={`agent-card${isActive ? " agent-card--active" : ""}`}
                  role="listitem"
                  data-testid={`agent-card-${a.id}`}
                >
                  <div className="agent-card__body">
                    <div className="agent-card__name">{a.id}</div>
                    <div className="agent-card__meta">
                      <span className="muted text-xs">{a.kind ?? "—"}</span>
                      {a.command && (
                        <>
                          <span className="agent-card__sep">·</span>
                          <code className="agent-card__command">{a.command}</code>
                        </>
                      )}
                    </div>
                    <div className={`agent-card__status agent-card__status--${a.status ?? "unknown"}`}>
                      {a.status ?? "—"}
                    </div>
                  </div>
                  <button
                    type="button"
                    className={`toggle${isActive ? " toggle--on" : ""}`}
                    role="switch"
                    aria-checked={isActive}
                    aria-label={`Toggle ${a.id}`}
                    data-testid={`agent-toggle-${a.id}`}
                    disabled={busy === a.id || a.kind === "in_process"}
                    onClick={() => void handleToggle(a.id, a.status)}
                  >
                    <span className="toggle__thumb" />
                  </button>
                </div>
              );
            })}
          </div>

          {state.loaded.agents && agents.length === 0 && (
            <EmptyState>
              <EmptyState.Title>No agents registered.</EmptyState.Title>
            </EmptyState>
          )}
        </>
      )}

      {tab === "skills" && <SkillsTab />}
      {tab === "tools" && <ToolsExplorerTab />}
    </Stack>
  );
}

/** EP-0024: contenido de SkillsPanel absorbido como tab interna. */
interface SkillItem {
  name: string;
  trigger_keywords: string[];
  instructions_preview: string;
  preferred_tools: string[];
}

function SkillsTab() {
  const [skills, setSkills] = useState<SkillItem[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await apiGet<{ skills?: SkillItem[] }>("/v1/skills");
      setSkills(data.skills ?? []);
    } catch {}
  }, []);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { void refresh(); }, [refresh]);

  return (
    <div className="skills-panel" data-testid="agents-skills-tab">
      <div className="skills-panel__header">
        <h2 className="skills-panel__title">Skills</h2>
        <span className="skills-panel__count">{skills.length} loaded</span>
      </div>

      <div className="skills-panel__list">
        {skills.map((sk) => {
          const isExpanded = expanded === sk.name;
          return (
            <div key={sk.name} className={`skills-panel__item${isExpanded ? " skills-panel__item--expanded" : ""}`}>
              <div className="skills-panel__item-header" onClick={() => setExpanded(isExpanded ? null : sk.name)}>
                <span className="skills-panel__item-icon"><IconDefaultAgent /></span>
                <span className="skills-panel__item-name">{sk.name}</span>
                <span className="skills-panel__item-keywords">
                  {sk.trigger_keywords.slice(0, 3).join(", ")}
                  {sk.trigger_keywords.length > 3 && ` +${sk.trigger_keywords.length - 3}`}
                </span>
                <span className={`skills-panel__item-chevron${isExpanded ? " skills-panel__item-chevron--open" : ""}`}>▾</span>
              </div>
              {isExpanded && (
                <div className="skills-panel__item-body">
                  <div className="skills-panel__item-section">
                    <span className="skills-panel__item-section-label">Triggers</span>
                    <div className="skills-panel__item-tags">
                      {sk.trigger_keywords.map((kw) => (
                        <span key={kw} className="skills-panel__tag">{kw}</span>
                      ))}
                    </div>
                  </div>
                  {sk.preferred_tools.length > 0 && (
                    <div className="skills-panel__item-section">
                      <span className="skills-panel__item-section-label">Preferred Tools</span>
                      <div className="skills-panel__item-tags">
                        {sk.preferred_tools.map((t) => (
                          <span key={t} className="skills-panel__tag skills-panel__tag--tool">{t}</span>
                        ))}
                      </div>
                    </div>
                  )}
                  <div className="skills-panel__item-section">
                    <span className="skills-panel__item-section-label">Instructions</span>
                    <p className="skills-panel__item-instructions">{sk.instructions_preview}</p>
                  </div>
                </div>
              )}
            </div>
          );
        })}
        {skills.length === 0 && (
          <div className="skills-panel__empty">No skills loaded</div>
        )}
      </div>
    </div>
  );
}