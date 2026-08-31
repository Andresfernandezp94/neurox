// PathChipsEditor — chips list with an inline input for adding paths.
//
// EP-0024-UX: replaces the textarea-based path editor with one row
// per path. Each chip shows the source path and, when ${workspace}
// is present, the resolved absolute path on hover (title). Chips
// can be removed individually. Below the chips there is a small
// input + Add button for new entries.
//
// Empty state shows a hint and a placeholder example. The preview
// (workspaceRoot) is optional — chips render fine without it (the
// tooltip just becomes the source path).

import { useCallback, useId, useState, type KeyboardEvent } from "react";

export interface PathChipsEditorProps {
  /** Current list of path entries (one chip per entry). */
  paths: string[];
  /** Called with the next list when the user adds / removes a chip. */
  onChange: (next: string[]) => void;
  /** Optional workspace root for ${workspace} expansion hints. */
  workspaceRoot?: string;
  /** Placeholder shown in the empty-state input. */
  placeholder?: string;
  /** data-testid prefix for the chips/buttons. */
  testIdPrefix?: string;
  /** If true, renders the input + Add button (default true). */
  editable?: boolean;
}

/** Resolve a single path entry against the workspace root. */
export function resolvePath(p: string, workspaceRoot: string): string {
  if (!workspaceRoot) return p;
  if (p === "${workspace}") return workspaceRoot;
  if (p.startsWith("${workspace}/")) {
    return `${workspaceRoot}/${p.slice("${workspace}/".length)}`;
  }
  return p;
}

export function PathChipsEditor({
  paths,
  onChange,
  workspaceRoot = "",
  placeholder = "${workspace}/.sdd",
  testIdPrefix = "path-chips",
  editable = true,
}: PathChipsEditorProps) {
  const [draft, setDraft] = useState("");
  const inputId = useId();

  const add = useCallback(
    (raw: string) => {
      const trimmed = raw.trim();
      if (!trimmed) return;
      if (paths.includes(trimmed)) {
        // de-dupe: don't add the same path twice
        setDraft("");
        return;
      }
      onChange([...paths, trimmed]);
      setDraft("");
    },
    [paths, onChange],
  );

  const remove = useCallback(
    (idx: number) => {
      onChange(paths.filter((_, i) => i !== idx));
    },
    [paths, onChange],
  );

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add(draft);
    } else if (e.key === "Backspace" && draft === "" && paths.length > 0) {
      // Backspace on empty input removes the last chip (sane UX).
      e.preventDefault();
      onChange(paths.slice(0, -1));
    }
  };

  return (
    <div className="path-chips" data-testid={testIdPrefix}>
      <ul className="path-chips__list" aria-label="Paths">
        {paths.length === 0 && (
          <li className="path-chips__empty muted">
            no paths — every read/write will be rejected under the sandbox
          </li>
        )}
        {paths.map((p, i) => {
          const resolved = resolvePath(p, workspaceRoot);
          const hasPlaceholder = workspaceRoot !== "" && p.startsWith("${workspace}");
          return (
            <li
              key={`${i}-${p}`}
              className="path-chips__item"
              data-testid={`${testIdPrefix}-item`}
              title={hasPlaceholder ? `${p}  →  ${resolved}` : p}
            >
              <code className="path-chips__source">{p}</code>
              {hasPlaceholder && (
                <span className="path-chips__arrow" aria-hidden="true">
                  →
                </span>
              )}
              {hasPlaceholder && (
                <code className="path-chips__resolved">{resolved}</code>
              )}
              {editable && (
                <button
                  type="button"
                  className="path-chips__remove"
                  data-testid={`${testIdPrefix}-remove`}
                  aria-label={`Remove ${p}`}
                  onClick={() => remove(i)}
                >
                  ×
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {editable && (
        <div className="path-chips__input-row">
          <input
            id={inputId}
            data-testid={`${testIdPrefix}-input`}
            className="input path-chips__input"
            type="text"
            value={draft}
            placeholder={placeholder}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKey}
          />
          <button
            type="button"
            className="button path-chips__add"
            data-testid={`${testIdPrefix}-add`}
            disabled={!draft.trim()}
            onClick={() => add(draft)}
          >
            + Add
          </button>
        </div>
      )}
    </div>
  );
}