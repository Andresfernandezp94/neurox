// WorkspaceViewer — tabbed view con Sandbox, Agents y MCP.
// EP-0026-UX: top-level workspace view con tabs internas.

import { useState } from "react";
import { Stack } from "../shared/components/molecules/Stack";
import { SandboxTab } from "./SandboxTab";
import { AgentsPanel } from "./AgentsPanel";
import { MCP } from "./MCP";
import { WorkspaceList, type WorkspaceConfig } from "./WorkspaceList";
import { WorkspaceConfigView } from "./WorkspaceConfigView";
import { useI18n } from "../shared/hooks/useI18n";

type WorkspaceTab = "general" | "sandbox" | "agents" | "mcp";

// Mock data — when `/v1/workspaces` is exposed by the daemon, replace
// this with a fetch + state.
const WORKSPACES: WorkspaceConfig[] = [
  {
    id: "sixbell",
    name: "Sixbell",
    description:
      "Sixbell product team workspace. 9 idempotent agents sharing the same sandbox, MCPs and memory pool.",
    status: "active",
    icon: "IconIntegrations",
    sandboxes: [
      {
        path: "/home/andres_fernandez/Sixbell/*",
        permissions: ["read", "write", "execute", "delete"],
        recursive: true,
      },
    ],
    network: {
      noNetwork: false,
      allow: [".*"],
      deny: [],
    },
    env: {
      SIXBELL_WORKSPACE: "true",
      SIXBELL_ROOT: "/home/andres_fernandez/Sixbell",
    },
    resources: {
      memoryMb: 4096,
      cpuCores: 4,
      diskMb: 10240,
      timeoutSecs: 120,
    },
    tools: {
      allow: ["*"],
      deny: ["rm -rf /"],
    },
    mcps: {
      memoryd: { enabled: true, config: { workspace: "sixbell" } },
      llmd: { enabled: true, config: {} },
      voice: { enabled: true, config: {} },
      clickup: { enabled: true, config: {} },
      playwright: { enabled: true, config: {} },
    },
    skills: [
      "ep-creator",
      "design-doc",
      "task-splitter",
    ],
    agents: [
      "developer",
      "doc-agent",
      "infra",
      "infrastructure",
      "orchestrator",
      "product-assistant",
      "researcher",
      "reviewer",
      "sql-qa",
    ],
  },
];

export function WorkspaceViewer() {
  const [tab, setTab] = useState<WorkspaceTab>("general");
  const [configWorkspaceId, setConfigWorkspaceId] = useState<string | null>(null);
  const { t } = useI18n();

  const configWorkspace = configWorkspaceId
    ? WORKSPACES.find((w) => w.id === configWorkspaceId) ?? null
    : null;

  if (configWorkspace) {
    return (
      <WorkspaceConfigView
        workspace={configWorkspace}
        onBack={() => setConfigWorkspaceId(null)}
      />
    );
  }

  return (
    <>
      <div className="config-viewer__tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "general"}
          data-testid="workspace-tab-general"
          className={`config-viewer__tab ${tab === "general" ? "active" : ""}`}
          onClick={() => setTab("general")}
        >
          General
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "sandbox"}
          data-testid="workspace-tab-sandbox"
          className={`config-viewer__tab ${tab === "sandbox" ? "active" : ""}`}
          onClick={() => setTab("sandbox")}
        >
          Sandbox
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "agents"}
          data-testid="workspace-tab-agents"
          className={`config-viewer__tab ${tab === "agents" ? "active" : ""}`}
          onClick={() => setTab("agents")}
        >
          {t("sidebar.agents")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "mcp"}
          data-testid="workspace-tab-mcp"
          className={`config-viewer__tab ${tab === "mcp" ? "active" : ""}`}
          onClick={() => setTab("mcp")}
        >
          MCP
        </button>
      </div>

      <div className="page-pad">
        <Stack gap="md">
          {tab === "general" && (
            <WorkspaceList
              workspaces={WORKSPACES}
              onAdd={() => {
                /* TODO: open a modal to create a new workspace */
              }}
              onDelete={(_id) => {
                /* TODO: confirm + delete workspace `_id` */
              }}
              onOpenConfig={(id) => setConfigWorkspaceId(id)}
            />
          )}
          {tab === "sandbox" && <SandboxTab />}
          {tab === "agents" && <AgentsPanel />}
          {tab === "mcp" && <MCP />}
        </Stack>
      </div>
    </>
  );
}
