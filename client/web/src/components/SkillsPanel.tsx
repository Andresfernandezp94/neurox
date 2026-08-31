// Skills Panel — shows all loaded skills with trigger keywords and instructions preview.

import { useState, useEffect, useCallback } from "react";
import { IconDefaultAgent } from "../shared/components/Icons";
import { apiGet } from "../api/client";

interface SkillItem {
  name: string;
  trigger_keywords: string[];
  instructions_preview: string;
  preferred_tools: string[];
}

export function SkillsPanel() {
  const [skills, setSkills] = useState<SkillItem[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await apiGet<{ skills?: SkillItem[] }>("/v1/skills");
      setSkills(data.skills ?? []);
    } catch {}
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  return (
    <div className="skills-panel">
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
