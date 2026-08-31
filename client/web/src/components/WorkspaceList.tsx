// WorkspaceList — lista de workspaces configurados.
// EP-0026-UX: tab "general" del WorkspaceViewer. Renderiza el shape
// rico de `Workspace` (sandbox, env, resources, mcps, model, etc).
//
// Por ahora la lista es client-side (mock data); cuando el backend
// exponga `/v1/workspaces` se conecta al endpoint.

import { useState } from "react";
import { Card } from "../shared/components/molecules/Card";
import { Row } from "../shared/components/molecules/Row";
import { Stack } from "../shared/components/molecules/Stack";
import { Icon } from "../shared/components/atoms/Icon";
import { IconButton } from "../shared/components/atoms/IconButton";
import { SearchBar } from "../shared/components/molecules/SearchBar";
import type { Workspace } from "./Workspace";

// Re-export for callers that still import `WorkspaceConfig` from here.
export type WorkspaceConfig = Workspace;

export interface WorkspaceListProps {
  workspaces: Workspace[];
  onAdd?: () => void;
  onDelete?: (id: string) => void;
  /** Initial search query (uncontrolled if `onSearchChange` is set). */
  search?: string;
  onSearchChange?: (value: string) => void;
  /** Called when the user clicks a card to open the config sub-view. */
  onOpenConfig?: (id: string) => void;
}

export function WorkspaceList({
  workspaces,
  onAdd,
  onDelete,
  search: searchProp,
  onSearchChange,
  onOpenConfig,
}: WorkspaceListProps) {
  const [internalSearch, setInternalSearch] = useState("");
  const isControlled = searchProp !== undefined && onSearchChange !== undefined;
  const search = isControlled ? searchProp : internalSearch;
  const setSearch = isControlled ? onSearchChange! : setInternalSearch;
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

      {workspaces.length === 0 ? (
        <Card>
          <div className="workspace-list__empty">
            <p className="muted">
              No workspaces configured yet.
            </p>
            <p className="muted text-sm">
              A workspace groups a <strong>sandbox</strong> (filesystem
              permissions), a set of <strong>active MCPs</strong> and a list
              of <strong>agents</strong>. Click <em>New workspace</em> to
              create one.
            </p>
          </div>
        </Card>
      ) : (
        <div className="workspace-list__items">
          {workspaces.map((w) => (
            <WorkspaceCard
              key={w.id}
              workspace={w}
              onOpen={() => onOpenConfig?.(w.id)}
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
  return (
    <button
      type="button"
      className="workspace-card__button"
      onClick={onOpen}
      data-testid={`workspace-card-${w.id}`}
    >
      <div className="workspace-card">
        <div className="workspace-card__header">
          <Row justify="between" align="center">
            <Row gap="sm" align="center">
              {w.icon && (
                <span className="workspace-card__icon" aria-hidden="true">
                  <Icon name={w.icon} size="md" />
                </span>
              )}
              <strong className="workspace-card__name">{w.name}</strong>
            </Row>
            {onDelete && (
              <IconButton
                icon="IconTrash"
                size="sm"
                variant="ghost"
                aria-label={`Delete ${w.name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete();
                }}
                data-testid={`workspace-delete-${w.id}`}
              />
            )}
          </Row>
          {w.description && (
            <p className="workspace-card__description">{w.description}</p>
          )}
        </div>

        <div className="workspace-card__count">
          {Object.values(w.mcps).filter((m) => m.enabled).length} MCPs ·{" "}
          {w.agents.length} Agents
        </div>
      </div>
    </button>
  );
}
