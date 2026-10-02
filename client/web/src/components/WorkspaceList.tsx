// WorkspaceList — lista de workspaces configurados.
// EP-0026-UX: tab "general" del WorkspaceViewer.
//
// Cada workspace es un entorno aislado: un directorio raiz y su propio
// sandbox, que se edita desde el form. La lista solo muestra y dispara
// las acciones; el estado y las llamadas viven en `WorkspaceViewer`.
//
// Antes la lista era presentacional sobre un array hardcodeado en
// `WorkspaceViewer` (el objeto "Sixbell"), con `onAdd` y `onDelete`
// cableados a handlers vacios.

import { useMemo, useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Stack } from "../shared/components/molecules/Stack";
import { EmptyState } from "../shared/components/molecules/EmptyState";
import { SearchBar } from "../shared/components/molecules/SearchBar";
import { Icon } from "../shared/components/atoms/Icon";
import { IconButton } from "../shared/components/atoms/IconButton";
import { Badge } from "../shared/components/atoms/Badge";
import { Code } from "../shared/components/atoms/Code";
import type { Workspace } from "../api/workspaces";

export type WorkspaceConfig = Workspace;

export interface WorkspaceListProps {
  workspaces: Workspace[];
  onAdd?: () => void;
  onDelete?: (id: string) => void;
  onOpenConfig?: (id: string) => void;
  search?: string;
  onSearchChange?: (value: string) => void;
  loading?: boolean;
  error?: string | null;
}

/** Coincede por nombre, descripcion, root o id: el root es lo que uno
 *  tiene a mano cuando busca ("cual era el de /srv/projects"). */
function matches(w: Workspace, q: string): boolean {
  if (!q) return true;
  const needle = q.toLowerCase();
  return (
    w.name.toLowerCase().includes(needle) ||
    (w.description ?? "").toLowerCase().includes(needle) ||
    w.root.toLowerCase().includes(needle) ||
    w.id.toLowerCase().includes(needle)
  );
}

export function WorkspaceList({
  workspaces,
  onAdd,
  onDelete,
  onOpenConfig,
  search: searchProp,
  onSearchChange,
  loading = false,
  error = null,
}: WorkspaceListProps) {
  const [internalSearch, setInternalSearch] = useState("");
  const isControlled = searchProp !== undefined && onSearchChange !== undefined;
  const search = isControlled ? searchProp : internalSearch;
  const setSearch = isControlled ? onSearchChange! : setInternalSearch;

  // El `search` se calculaba y se pasaba al SearchBar pero nunca se usaba
  // para filtrar: la lista renderizaba el array entero.
  const visibles = useMemo(
    () => workspaces.filter((w) => matches(w, search)),
    [workspaces, search],
  );

  return (
    <Stack gap="md" data-testid="workspace-list">
      <div className="workspace-list__header">
        <h2 className="workspace-list__title">Workspaces</h2>
        {onAdd && (
          <IconButton
            icon="IconPlus"
            size="sm"
            variant="default"
            aria-label="New workspace"
            onClick={onAdd}
            data-testid="workspace-add"
          />
        )}
      </div>

      <SearchBar
        value={search}
        onChange={setSearch}
        placeholder="Search workspaces…"
        ariaLabel="Search workspaces"
        className="workspace-list__search"
      />

      {error != null && (
        <p className="workspace-list__error" role="alert" data-testid="workspace-error">
          {error}
        </p>
      )}

      {loading && workspaces.length === 0 ? (
        <p className="muted workspace-list__hint">Loading workspaces…</p>
      ) : workspaces.length === 0 ? (
        <Card className="workspace-list__empty">
          <EmptyState>
            <EmptyState.Icon>
              <Icon name="IconGrid" />
            </EmptyState.Icon>
            <EmptyState.Title>No workspaces configured yet.</EmptyState.Title>
            <EmptyState.Hint>
              A workspace is an isolated environment: its own directory, its own sandbox
              permissions, and its own agents. Click New workspace to create one.
            </EmptyState.Hint>
          </EmptyState>
        </Card>
      ) : visibles.length === 0 ? (
        <Card className="workspace-list__empty">
          <EmptyState>
            <EmptyState.Title>No matches.</EmptyState.Title>
            <EmptyState.Hint>
              Nothing matches “{search}”. Search looks at name, description, root and id.
            </EmptyState.Hint>
          </EmptyState>
        </Card>
      ) : (
        <div className="workspace-list__items">
          {visibles.map((w) => (
            <WorkspaceCard
              key={w.id}
              workspace={w}
              onOpen={onOpenConfig ? () => onOpenConfig(w.id) : undefined}
              onDelete={onDelete ? () => onDelete(w.id) : undefined}
            />
          ))}
        </div>
      )}
    </Stack>
  );
}

function WorkspaceCard({
  workspace: w,
  onOpen,
  onDelete,
}: {
  workspace: Workspace;
  onOpen?: () => void;
  onDelete?: () => void;
}) {
  const total = w.sandbox.readable_paths.length + w.sandbox.writable_paths.length;
  return (
    // `shell` posiciona el boton de borrar como hermano del boton
    // principal. Antes la card entera era un <button> y el IconButton de
    // borrar quedaba DENTRO (HTML invalido: un boton no puede contener
    // otro) y el click se perdia. El CSS de este layout ya existia; el
    // TSX nunca lo habia usado.
    <Card className="workspace-card" data-testid={`workspace-card-${w.id}`}>
      <div className="workspace-card__shell">
        <button
          type="button"
          className="workspace-card__button"
          onClick={onOpen}
          disabled={!onOpen}
          data-testid={`workspace-card-open-${w.id}`}
        >
          <span className="workspace-card__top">
            <span className="workspace-card__icon" aria-hidden="true">
              <Icon name={(w.icon as never) ?? "IconWorkspace"} size="md" />
            </span>
            <span className="workspace-card__name">{w.name}</span>
            {w.is_default && (
              <Badge variant="info" dot>
                default
              </Badge>
            )}
            {w.status !== "active" && <Badge variant="neutral">{w.status}</Badge>}
          </span>

          {w.description && <p className="workspace-card__description">{w.description}</p>}

          <div className="workspace-card__root">
            <Code>{w.root}</Code>
          </div>

          <div className="workspace-card__counts">
            <span>{w.sandbox.readable_paths.length} readable</span>
            <span>{w.sandbox.writable_paths.length} writable</span>
            <span>depth {w.sandbox.max_recursion_depth}</span>
            <span>{total === 0 ? "no sandbox paths" : "sandbox on"}</span>
          </div>
        </button>

        {onDelete && (
          <div className="workspace-card__delete">
            <IconButton
              icon="IconTrash"
              size="sm"
              variant="ghost"
              aria-label={`Delete ${w.name}`}
              onClick={onDelete}
              data-testid={`workspace-delete-${w.id}`}
            />
          </div>
        )}
      </div>
    </Card>
  );
}