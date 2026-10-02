import { useEffect, useState } from "react";
import { Modal } from "../shared/components/molecules/Modal";
import { Stack } from "../shared/components/molecules/Stack";
import { Button } from "../shared/components/atoms/Button";
import { Input } from "../shared/components/atoms/Input";
import { PathChipsEditor } from "../shared/components/molecules/PathChipsEditor";
import type { SandboxDefaults, Workspace } from "../api/workspaces";

export interface WorkspaceFormValue {
  name: string;
  root: string;
  description: string;
  status: Workspace["status"];
  sandbox_enabled: boolean;
  readable_paths: string[];
  writable_paths: string[];
  max_recursion_depth: number;
}

export interface WorkspaceFormModalProps {
  open: boolean;
  mode: "create" | "edit";
  workspace?: Workspace | null;
  /** Defaults del daemon, para no hardcodear `${workspace}` aca. */
  defaults?: SandboxDefaults | null;
  saving?: boolean;
  onClose: () => void;
  onSubmit: (value: WorkspaceFormValue) => void;
}

/** Los depth validos son los del backend (1..100). */
const MIN_DEPTH = 1;
const MAX_DEPTH = 100;

/**
 * Form de un workspace: lo esencial y nada mas.
 *
 * Se dejo afuera la vista de 9 secciones (network, resources, mcps, skills,
 * tools) a proposito: el backend no puede aplicar nada de eso hoy, asi que
 * mostrarla daria la sensacion de que aisla cuando no aisla. Cuando exista
 * enforcement de red o de recursos, se suma.
 */
export function WorkspaceFormModal({
  open,
  mode,
  workspace,
  defaults,
  saving = false,
  onClose,
  onSubmit,
}: WorkspaceFormModalProps) {
  const defaultReadable = defaults?.defaults.readable_paths ?? ["${workspace}"];
  const defaultDepth = defaults?.defaults.max_recursion_depth ?? 10;
  const defaultEnabled = defaults?.defaults.enabled ?? true;

  const [name, setName] = useState("");
  const [root, setRoot] = useState("");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<Workspace["status"]>("active");
  const [enabled, setEnabled] = useState(defaultEnabled);
  const [readable, setReadable] = useState<string[]>(defaultReadable);
  const [writable, setWritable] = useState<string[]>([]);
  const [depth, setDepth] = useState(defaultDepth);

  // El form se rehidrata cuando cambia el workspace o cuando se abre, para
  // que editar dos seguidos no arrastre los valores del anterior.
  useEffect(() => {
    if (!open) return;
    if (mode === "edit" && workspace) {
      setName(workspace.name);
      setRoot(workspace.root);
      setDescription(workspace.description ?? "");
      setStatus(workspace.status);
      setEnabled(workspace.sandbox.enabled);
      setReadable(workspace.sandbox.readable_paths);
      setWritable(workspace.sandbox.writable_paths);
      setDepth(workspace.sandbox.max_recursion_depth);
    } else {
      setName("");
      setRoot("");
      setDescription("");
      setStatus("active");
      setEnabled(defaultEnabled);
      setReadable(defaultReadable);
      setWritable([]);
      setDepth(defaultDepth);
    }
  }, [open, mode, workspace, defaultEnabled, defaultReadable, defaultDepth]);

  // El root del workspace es lo que resuelve `${workspace}` en los paths, asi
  // que el editor de chips lo necesita para mostrar la resolucion.
  const rootHint = root || "the workspace root";

  const puedeGuardar = name.trim() !== "" && root.trim() !== "" && !saving;

  const submit = () => {
    if (!puedeGuardar) return;
    onSubmit({
      name: name.trim(),
      root: root.trim(),
      description: description.trim(),
      status,
      sandbox_enabled: enabled,
      readable_paths: readable,
      writable_paths: writable,
      max_recursion_depth: depth,
    });
  };

  return (
    <Modal
      open={open}
      className="modal--wide"
      onClose={onClose}
      title={mode === "create" ? "New workspace" : "Edit workspace"}
      data-testid="workspace-form"
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={submit}
            disabled={!puedeGuardar}
            loading={saving}
            data-testid="workspace-form-save"
          >
            {mode === "create" ? "Create" : "Save changes"}
          </Button>
        </>
      }
    >
      <Stack gap="md">
        <div className="workspace-form__row">
          <label className="workspace-form__label" htmlFor="workspace-form-name">
            Name
          </label>
          <Input
            id="workspace-form-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Sixbell"
            data-testid="workspace-form-name"
          />
        </div>

        <div className="workspace-form__row">
          <label className="workspace-form__label" htmlFor="workspace-form-root">
            Root directory
          </label>
          <Input
            id="workspace-form-root"
            value={root}
            onChange={(e) => setRoot(e.target.value)}
            placeholder="/home/andres_fernandez/Sixbell"
            data-testid="workspace-form-root"
          />
          <p className="workspace-form__hint">
            Absolute path. It has to be absolute: a relative root does not isolate anything,
            because each tool would resolve it against its own cwd. The directory is not created
            for you.
          </p>
        </div>

        <div className="workspace-form__row">
          <label className="workspace-form__label" htmlFor="workspace-form-description">
            Description
          </label>
          <Input
            id="workspace-form-description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What lives here and which rules apply"
            data-testid="workspace-form-description"
          />
        </div>

        <div className="workspace-form__row">
          <label className="workspace-form__label" htmlFor="workspace-form-status">
            Status
          </label>
          <select
            id="workspace-form-status"
            className="workspace-form__select"
            value={status}
            onChange={(e) => setStatus(e.target.value as Workspace["status"])}
            data-testid="workspace-form-status"
          >
            <option value="active">active</option>
            <option value="paused">paused</option>
            <option value="draft">draft</option>
          </select>
        </div>

        <div className="workspace-form__row workspace-form__row--switch">
          <label className="workspace-form__label" htmlFor="workspace-form-enabled">
            Sandbox enforced
          </label>
          <input
            id="workspace-form-enabled"
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
            data-testid="workspace-form-enabled"
          />
        </div>

        <div className="workspace-form__section">
          <span className="workspace-form__section-title">Readable paths</span>
          <PathChipsEditor
            paths={readable}
            onChange={setReadable}
            workspaceRoot={rootHint}
            testIdPrefix="workspace-form-readable"
          />
        </div>

        <div className="workspace-form__section">
          <span className="workspace-form__section-title">Writable paths</span>
          <PathChipsEditor
            paths={writable}
            onChange={setWritable}
            workspaceRoot={rootHint}
            testIdPrefix="workspace-form-writable"
          />
          <p className="workspace-form__hint">
            Empty means nothing is writable in this workspace, not even the root. Use
            <code>{" ${workspace}"}</code> to open the root itself.
          </p>
        </div>

        <div className="workspace-form__row">
          <label className="workspace-form__label" htmlFor="workspace-form-depth">
            Max recursion depth
          </label>
          <Input
            id="workspace-form-depth"
            type="number"
            min={MIN_DEPTH}
            max={MAX_DEPTH}
            value={depth}
            onChange={(e) =>
              setDepth(
                Math.max(MIN_DEPTH, Math.min(MAX_DEPTH, parseInt(e.target.value, 10) || MIN_DEPTH)),
              )
            }
            data-testid="workspace-form-depth"
          />
          <p className="workspace-form__hint">
            How deep <code>glob</code> / <code>grep</code> descend ({MIN_DEPTH}–{MAX_DEPTH}).
          </p>
        </div>
      </Stack>
    </Modal>
  );
}