// WorkspaceConfigView — sub-vista de configuración de un workspace.
// EP-0026-UX: se muestra como una vista normal (no modal) cuando el
// usuario hace click en una card de WorkspaceList. Tiene un botón
// "back" para volver a la lista.
//
// Las secciones son toggles colapsables (header = título, body = configs).
// Toda la página se scrollea verticalmente.

import { useState } from "react";
import { Stack } from "../shared/components/molecules/Stack";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { IconButton } from "../shared/components/atoms/IconButton";
import { Icon } from "../shared/components/atoms/Icon";
import { Badge } from "../shared/components/atoms/Badge";
import { Button } from "../shared/components/atoms/Button";
import type { Workspace, SandboxRule, McpConfig } from "./Workspace";

export interface WorkspaceConfigViewProps {
  workspace: Workspace;
  onBack: () => void;
  onSave?: (updated: Workspace) => void;
}

type Section = "basics" | "sandbox" | "network" | "env" | "resources" | "tools" | "mcps" | "skills" | "agents";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "basics", label: "Basics" },
  { id: "sandbox", label: "Sandbox" },
  { id: "network", label: "Network" },
  { id: "env", label: "Environment" },
  { id: "resources", label: "Resources" },
  { id: "tools", label: "Tools" },
  { id: "mcps", label: "MCPs" },
  { id: "skills", label: "Skills" },
  { id: "agents", label: "Agents" },
];

export function WorkspaceConfigView({
  workspace,
  onBack,
  onSave,
}: WorkspaceConfigViewProps) {
  const [expanded, setExpanded] = useState<Set<Section>>(
    () => new Set<Section>(["basics"]),
  );

  const toggle = (id: Section) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  return (
    <div className="workspace-config" data-testid="workspace-config-view">
      <header className="workspace-config__header">
        <Row gap="sm" align="center">
          <IconButton
            icon="IconChevron"
            size="sm"
            variant="ghost"
            aria-label="Back to workspaces"
            onClick={onBack}
            data-testid="workspace-config-back"
          />
          {workspace.icon && (
            <span className="workspace-config__icon" aria-hidden="true">
              <Icon name={workspace.icon} size="md" />
            </span>
          )}
          <div>
            <h2 className="workspace-config__title">{workspace.name}</h2>
            {workspace.description && (
              <p className="muted text-sm">{workspace.description}</p>
            )}
          </div>
        </Row>
        <Button
          variant="primary"
          size="sm"
          onClick={() => onSave?.(workspace)}
          disabled={!onSave}
        >
          Save changes
        </Button>
      </header>

      <div className="workspace-config__body">
        <Stack gap="md">
          {SECTIONS.map((s) => {
            const isOpen = expanded.has(s.id);
            return (
              <Card key={s.id} className="workspace-modal__section">
                <button
                  type="button"
                  className="workspace-modal__section-toggle"
                  aria-expanded={isOpen}
                  aria-controls={`workspace-section-body-${s.id}`}
                  onClick={() => toggle(s.id)}
                  data-testid={`workspace-section-toggle-${s.id}`}
                >
                  <span className="workspace-modal__section-label">
                    {s.label}
                  </span>
                  <span
                    className={`workspace-modal__chevron ${isOpen ? "is-open" : ""}`}
                    aria-hidden="true"
                  >
                    ▾
                  </span>
                </button>
                {isOpen && (
                  <div
                    id={`workspace-section-body-${s.id}`}
                    className="workspace-modal__section-body"
                  >
                    {s.id === "basics" && <BasicsSection workspace={workspace} />}
                    {s.id === "sandbox" && <SandboxSection rules={workspace.sandboxes} />}
                    {s.id === "network" && <NetworkSection policy={workspace.network} />}
                    {s.id === "env" && <EnvSection env={workspace.env} />}
                    {s.id === "resources" && <ResourcesSection r={workspace.resources} />}
                    {s.id === "tools" && <ToolsSection t={workspace.tools} />}
                    {s.id === "mcps" && <McpsSection mcps={workspace.mcps} />}
                    {s.id === "skills" && <SkillsSection skills={workspace.skills} />}
                    {s.id === "agents" && <AgentsSection agents={workspace.agents} />}
                  </div>
                )}
              </Card>
            );
          })}
        </Stack>
      </div>
    </div>
  );
}

/* ─── Sections ─────────────────────────────────────────────── */

function BasicsSection({ workspace }: { workspace: Workspace }) {
  return (
    <Stack gap="md">
      <Field label="Name" value={workspace.name} />
      <Field label="ID" value={workspace.id} mono />
      <Field label="Description" value={workspace.description ?? "—"} />
      <Field
        label="Status"
        value={
          <Badge
            variant={
              workspace.status === "active"
                ? "success"
                : workspace.status === "paused"
                ? "warn"
                : "neutral"
            }
          >
            {workspace.status ?? "draft"}
          </Badge>
        }
      />
      <Field label="Parent (inherits from)" value={workspace.parent ?? "—"} />
    </Stack>
  );
}

function SandboxSection({ rules }: { rules: SandboxRule[] }) {
  return (
    <Stack gap="md">
      {rules.length === 0 ? (
        <Empty label="No sandbox rules. Click + to add one." />
      ) : (
        rules.map((r, i) => (
          <Card key={`${r.path}-${i}`} className="workspace-modal__rule">
            <Stack gap="sm">
              <code className="workspace-modal__path">{r.path}</code>
              <Row gap="sm" wrap>
                {r.permissions.map((p) => (
                  <Badge key={p} variant="info">
                    {p}
                  </Badge>
                ))}
                {r.recursive && <Badge variant="neutral">recursive</Badge>}
              </Row>
            </Stack>
          </Card>
        ))
      )}
    </Stack>
  );
}

function NetworkSection({
  policy,
}: {
  policy: Workspace["network"];
}) {
  return (
    <Stack gap="md">
      <Field
        label="No network"
        value={policy.noNetwork ? "Yes (offline)" : "No"}
      />
      <Field
        label="Allow (regex)"
        value={policy.allow?.length ? policy.allow.join(", ") : "—"}
        mono
      />
      <Field
        label="Deny (regex)"
        value={policy.deny?.length ? policy.deny.join(", ") : "—"}
        mono
      />
    </Stack>
  );
}

function EnvSection({ env }: { env: Workspace["env"] }) {
  const entries = Object.entries(env);
  return (
    <Stack gap="md">
      {entries.length === 0 ? (
        <Empty label="No env vars. Click + to add one." />
      ) : (
        entries.map(([k, v]) => (
          <div key={k} className="workspace-modal__env-row">
            <code className="workspace-modal__env-key">{k}</code>
            <code className="workspace-modal__env-val">
              {typeof v === "string" ? v : "••• (secret)"}
            </code>
          </div>
        ))
      )}
    </Stack>
  );
}

function ResourcesSection({
  r,
}: {
  r: Workspace["resources"];
}) {
  return (
    <Stack gap="md">
      <Field label="Memory" value={r.memoryMb ? `${r.memoryMb} MB` : "—"} />
      <Field label="CPU cores" value={r.cpuCores ? String(r.cpuCores) : "—"} />
      <Field label="Disk" value={r.diskMb ? `${r.diskMb} MB` : "—"} />
      <Field label="Timeout" value={r.timeoutSecs ? `${r.timeoutSecs} s` : "—"} />
    </Stack>
  );
}

function ToolsSection({ t }: { t: Workspace["tools"] }) {
  return (
    <Stack gap="md">
      <Field
        label="Allow"
        value={t.allow?.length ? t.allow.join(", ") : "—"}
        mono
      />
      <Field
        label="Deny"
        value={t.deny?.length ? t.deny.join(", ") : "—"}
        mono
      />
    </Stack>
  );
}

function McpsSection({ mcps }: { mcps: Workspace["mcps"] }) {
  const entries = Object.entries(mcps);
  return (
    <Stack gap="md">
      {entries.length === 0 ? (
        <Empty label="No MCPs activated." />
      ) : (
        entries.map(([name, m]) => <McpCard key={name} name={name} cfg={m} />)
      )}
    </Stack>
  );
}

function McpCard({ name, cfg }: { name: string; cfg: McpConfig }) {
  return (
    <Card className="workspace-modal__mcp">
      <Row justify="between" align="center">
        <Row gap="sm" align="center">
          <strong>{name}</strong>
          <Badge variant={cfg.enabled ? "success" : "neutral"}>
            {cfg.enabled ? "enabled" : "disabled"}
          </Badge>
        </Row>
      </Row>
      {Object.keys(cfg.config).length > 0 && (
        <pre className="workspace-modal__mcp-config">
          {JSON.stringify(cfg.config, null, 2)}
        </pre>
      )}
    </Card>
  );
}

function SkillsSection({ skills }: { skills: string[] }) {
  return (
    <Stack gap="md">
      {skills.length === 0 ? (
        <Empty label="No skills configured." />
      ) : (
        <Row gap="sm" wrap>
          {skills.map((s) => (
            <Badge key={s} variant="neutral">
              {s}
            </Badge>
          ))}
        </Row>
      )}
    </Stack>
  );
}

function AgentsSection({ agents }: { agents: string[] }) {
  return (
    <Stack gap="md">
      {agents.length === 0 ? (
        <Empty label="No agents in this workspace." />
      ) : (
        <Row gap="sm" wrap>
          {agents.map((a) => (
            <Badge key={a} variant="success">
              {a}
            </Badge>
          ))}
        </Row>
      )}
    </Stack>
  );
}

function Field({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="workspace-modal__field">
      <span className="workspace-modal__field-label">{label}</span>
      <span
        className={`workspace-modal__field-value ${mono ? "text-mono" : ""}`}
      >
        {value}
      </span>
    </div>
  );
}

function Empty({ label }: { label: string }) {
  return <p className="muted workspace-modal__empty">{label}</p>;
}
