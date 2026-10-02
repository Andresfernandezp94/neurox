import { useCallback, useEffect, useState } from "react";
import { Stack } from "../shared/components/molecules/Stack";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Tabs } from "../shared/components/molecules/Tabs";
import { SandboxTab } from "./SandboxTab";
import { AgentsPanel } from "./AgentsPanel";
import { MCP } from "./MCP";
import { WorkspaceList } from "./WorkspaceList";
import { WorkspaceFormModal, type WorkspaceFormValue } from "./WorkspaceFormModal";
import { ConfirmDialog } from "../shared/components/ConfirmDialog";
import {
  createWorkspace,
  deleteWorkspace,
  getSandboxDefaults,
  listWorkspaces,
  updateWorkspace,
  type SandboxDefaults,
  type Workspace,
} from "../api/workspaces";

type WorkspaceTab = "general" | "sandbox" | "agents" | "mcp";

// WorkspaceViewer — tabbed view con General, Sandbox, Agents y MCP.
//
// La tab General lista los workspaces contra `/v1/workspaces`. Antes era un
// objeto "Sixbell" hardcodeado a nivel de modulo (const WORKSPACES), sin
// estado ni fetch: agregar o borrar no hacia nada porque los handlers eran
// no-ops con un TODO, y el boton "Save changes" de la vista de config estaba
// permanentemente deshabilitado.
//
// El fetch usa el patron de los otros paneles (`useState` + `useCallback` +
// `useEffect` + `ErrorBanner` al tope, ver EnvTab).
export function WorkspaceViewer() {
  const [tab, setTab] = useState<WorkspaceTab>("general");

  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [defaults, setDefaults] = useState<SandboxDefaults | null>(null);

  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Workspace | null>(null);
  const [creating, setCreating] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<Workspace | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [list, defs] = await Promise.all([
        listWorkspaces(),
        getSandboxDefaults().catch(() => null),
      ]);
      setWorkspaces(list);
      setDefaults(defs);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const onCreate = async (value: WorkspaceFormValue) => {
    setSaving(true);
    try {
      await createWorkspace({
        name: value.name,
        root: value.root,
        description: value.description || null,
        status: value.status,
        sandbox_enabled: value.sandbox_enabled,
        sandbox_readable_paths: value.readable_paths,
        sandbox_writable_paths: value.writable_paths,
        sandbox_max_recursion_depth: value.max_recursion_depth,
      });
      setCreating(false);
      await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const onSaveEdit = async (value: WorkspaceFormValue) => {
    if (!editing) return;
    setSaving(true);
    try {
      await updateWorkspace(editing.id, {
        name: value.name,
        root: value.root,
        description: value.description || null,
        status: value.status,
        sandbox_enabled: value.sandbox_enabled,
        sandbox_readable_paths: value.readable_paths,
        sandbox_writable_paths: value.writable_paths,
        sandbox_max_recursion_depth: value.max_recursion_depth,
      });
      setEditing(null);
      await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const onDelete = async () => {
    if (!pendingDelete) return;
    const id = pendingDelete.id;
    setPendingDelete(null);
    try {
      await deleteWorkspace(id);
      await load();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div className="page-pad">
      <Stack gap="md">
        {error != null && <ErrorBanner onDismiss={() => setError(null)}>{error}</ErrorBanner>}

        <Tabs
          items={[
            { id: "general" as const, label: "General" },
            { id: "sandbox" as const, label: "Sandbox" },
            { id: "agents" as const, label: "Agentes" },
            { id: "mcp" as const, label: "MCP" },
          ]}
          active={tab}
          onChange={setTab}
          testIdPrefix="workspace-tab"
        />

        {tab === "general" && (
          <WorkspaceList
            workspaces={workspaces}
            loading={loading}
            error={null}
            search={search}
            onSearchChange={setSearch}
            onAdd={() => setCreating(true)}
            onOpenConfig={(id) => setEditing(workspaces.find((w) => w.id === id) ?? null)}
            onDelete={(id) => setPendingDelete(workspaces.find((w) => w.id === id) ?? null)}
          />
        )}
        {tab === "sandbox" && <SandboxTab />}
        {tab === "agents" && <AgentsPanel />}
        {tab === "mcp" && <MCP />}
      </Stack>

      <WorkspaceFormModal
        open={creating}
        mode="create"
        defaults={defaults}
        saving={saving}
        onClose={() => setCreating(false)}
        onSubmit={onCreate}
      />

      <WorkspaceFormModal
        open={editing != null}
        mode="edit"
        workspace={editing}
        defaults={defaults}
        saving={saving}
        onClose={() => setEditing(null)}
        onSubmit={onSaveEdit}
      />

      <ConfirmDialog
        open={pendingDelete != null}
        title={pendingDelete ? `Remove “${pendingDelete.name}”?` : undefined}
        destructive
        message={
          pendingDelete
            ? `The workspace is removed and the agents lose access to ${pendingDelete.root}. The directory and everything in it are NOT deleted.`
            : ""
        }
        confirmLabel="Remove"
        onConfirm={onDelete}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
}