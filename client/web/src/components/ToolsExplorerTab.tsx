// Tools Explorer v3 — vertical list per category, SVG icons, status dots, inline test.
// Layout per tool row:
//   icon  tool_name                    ● status   ▾ toggle
//   ─────────────────────────────────────────────────────
//   description (left col)     |  test area (right col)

import { useState, useMemo, useCallback, type ReactNode } from "react";
import { listTools } from "../api/tools";
import { usePolling } from "../hooks/usePolling";
import {
  IconFolder, IconTerminal, IconDefaultAgent, IconGlobe,
  IconMonitor, IconSparkles, IconLink, IconZap,
} from "../shared/components/Icons";
import type { ToolsResponse, ToolSpec } from "../types";

// ─── Categories ──────────────────────────────────────────────────────────────

interface ToolCategory {
  id: string;
  label: string;
  icon: () => ReactNode;
  match: (name: string) => boolean;
}

const CATEGORIES: ToolCategory[] = [
  { id: "files", label: "Files & Code", icon: IconFolder, match: (n) => /^(read_file|write_file|list_dir|glob|grep|symbols)$/.test(n) },
  { id: "shell", label: "Shell", icon: IconTerminal, match: (n) => n === "shell" },
  { id: "memory", label: "Memory", icon: IconDefaultAgent, match: (n) => /^(memory_|save_fact|search_memory|knowledge)/.test(n) },
  { id: "web", label: "Web", icon: IconGlobe, match: (n) => /^(web_fetch|web_search)$/.test(n) },
  { id: "desktop", label: "Desktop", icon: IconMonitor, match: (n) => /^(clipboard_|screenshot|ocr_screen)/.test(n) },
  { id: "generation", label: "Generation", icon: IconSparkles, match: (n) => /^generate_/.test(n) },
  // Por convencion de nombre, como las demas categorias. Antes comparaba
  // contra un unico nombre hardcodeado que no era un tool del daemon, con
  // lo que la grupo nunca tenia nada y ni se renderizaba.
  { id: "delegation", label: "Delegation", icon: IconLink, match: (n) => /^(delegate|delegation|spawn_agent|subagent)/.test(n) },
  { id: "other", label: "Other", icon: IconZap, match: () => true },
];

function categorize(tools: ToolSpec[]): Map<string, ToolSpec[]> {
  const map = new Map<string, ToolSpec[]>();
  for (const cat of CATEGORIES) map.set(cat.id, []);
  for (const tool of tools) {
    const fallback = CATEGORIES[CATEGORIES.length - 1];
    const cat = CATEGORIES.find((c) => c.match(tool.name)) ?? fallback;
    if (cat) map.get(cat.id)?.push(tool);
  }
  return map;
}

// ─── Main Component ──────────────────────────────────────────────────────────

export function ToolsExplorerTab() {
  const { data } = usePolling<ToolsResponse>(listTools, 30_000);
  const [expandedTool, setExpandedTool] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const tools = data?.tools ?? [];
  const grouped = useMemo(() => categorize(tools), [tools]);

  const toggleCategory = useCallback((catId: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(catId)) next.delete(catId); else next.add(catId);
      return next;
    });
  }, []);

  const toggleTool = useCallback((name: string) => {
    setExpandedTool((prev) => prev === name ? null : name);
  }, []);

  return (
    <div className="te" data-testid="config-tools-explorer-tab">
      <div className="te__header">
        <h2 className="te__title">Tools</h2>
        <span className="te__count">{tools.length} registered</span>
      </div>

      <div className="te__cards">
        {CATEGORIES.map((cat) => {
          const items = grouped.get(cat.id) ?? [];
          if (items.length === 0) return null;
          const isCollapsed = collapsed.has(cat.id);
          const CatIcon = cat.icon;
          return (
            <div key={cat.id} className="te__card">
              <div className="te__card-header" onClick={() => toggleCategory(cat.id)}>
                <span className="te__card-icon"><CatIcon /></span>
                <span className="te__card-label">{cat.label}</span>
                <span className="te__card-count">{items.length}</span>
                <span className={`te__card-chevron${isCollapsed ? "" : " te__card-chevron--open"}`}>▾</span>
              </div>
              {!isCollapsed && (
                <div className="te__tool-list">
                  {items.map((t) => (
                    <ToolRow
                      key={t.name}
                      tool={t}
                      expanded={expandedTool === t.name}
                      onToggle={() => toggleTool(t.name)}
                    />
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ─── Tool Row ────────────────────────────────────────────────────────────────

interface ToolRowProps {
  tool: ToolSpec;
  expanded: boolean;
  onToggle: () => void;
}

function ToolRow({ tool, expanded, onToggle }: ToolRowProps) {
  return (
    <div className={`te__tool${expanded ? " te__tool--expanded" : ""}`}>
      {/* Header row: name + status dot + toggle */}
      <div className="te__tool-header" onClick={onToggle}>
        <span className="te__tool-name">{tool.name}</span>
        {tool.requires_approval && <span className="te__tool-lock">🔒</span>}
        <span className="te__tool-dot" />
        <span className={`te__tool-toggle${expanded ? " te__tool-toggle--open" : ""}`}>▾</span>
      </div>

      {/* Expanded: two-column — description | test */}
      {expanded && <ToolDetail tool={tool} />}
    </div>
  );
}

// ─── Tool Detail (two columns: info | test) ──────────────────────────────────

function ToolDetail({ tool }: { tool: ToolSpec }) {
  const [argsText, setArgsText] = useState("{}");
  const [output, setOutput] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const properties = (tool.parameters as { properties?: Record<string, unknown> })?.properties ?? {};
  const required = ((tool.parameters as { required?: string[] })?.required) ?? [];
  const propertyNames = Object.keys(properties);

  const handleRun = useCallback(async () => {
    setRunning(true);
    setOutput(null);
    try {
      const args = JSON.parse(argsText) as Record<string, unknown>;
      setOutput(`Preview — would invoke ${tool.name} with:\n${JSON.stringify(args, null, 2)}`);
    } catch (e) {
      setOutput(`Invalid JSON: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setRunning(false);
    }
  }, [argsText, tool.name]);

  return (
    <div className="te__detail">
      {/* Left column: description + params */}
      <div className="te__detail-info">
        <p className="te__detail-desc">{tool.description}</p>
        {propertyNames.length > 0 && (
          <div className="te__detail-params">
            {propertyNames.map((p) => {
              const prop = properties[p] as Record<string, unknown> | undefined;
              const type = (prop?.type as string) ?? "any";
              const desc = (prop?.description as string) ?? "";
              const isReq = required.includes(p);
              return (
                <div key={p} className="te__detail-param">
                  <span className="te__detail-param-name">{p}</span>
                  <span className="te__detail-param-type">{type}</span>
                  {isReq && <span className="te__detail-param-req">required</span>}
                  {desc && <div className="te__detail-param-desc">{desc}</div>}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Right column: test */}
      <div className="te__detail-test">
        <textarea
          className="te__detail-textarea"
          value={argsText}
          onChange={(e) => setArgsText(e.target.value)}
          rows={3}
          placeholder='{"arg": "value"}'
        />
        <button className="te__detail-run" onClick={() => void handleRun()} disabled={running}>
          ▶ Test
        </button>
        {output && <pre className="te__detail-output">{output}</pre>}
      </div>
    </div>
  );
}
