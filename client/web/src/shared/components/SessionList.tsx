// SessionList — Componente compartido para listas de sesiones.
// Variants: "sidebar" | "panel" | "expanded"

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { IconClipboard, IconSearch, IconTrash } from "./Icons";
import { ConfirmDialog } from "./ConfirmDialog";
import type { SessionSummary } from "../../types";

export type SessionListVariant = "sidebar" | "panel" | "expanded";
export type SessionStatusFilter = "all" | "active" | "closed";
export type SessionSortKey = "started_at" | "agent_id" | "status";
export type SessionSortDir = "asc" | "desc";

export interface SessionListProps {
  sessions: SessionSummary[];
  currentSessionId?: string | null;
  variant?: SessionListVariant;
  loading?: boolean;
  onLoad?: (sessionId: string) => void;
  onRename?: (sessionId: string, name: string) => void | Promise<void>;
  onDelete?: (sessionId: string) => void | Promise<void>;
  /** EP-0028-b: stop the running agent for this session. The session
   *  record stays in the list (status dot goes from green to grey).
   *  When omitted, no stop button is rendered. */
  onStop?: (sessionId: string) => void | Promise<void>;
  className?: string;
  headerLabel?: string;
  headerAction?: React.ReactNode;
}

function formatDateShort(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return (
    d.toLocaleDateString(undefined, { month: "short", day: "numeric" }) +
    " " +
    d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
  );
}

function formatDateLong(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function computeDuration(start: string, end: string | null | undefined): string {
  const s = new Date(start).getTime();
  const e = end ? new Date(end).getTime() : Date.now();
  if (Number.isNaN(s) || Number.isNaN(e) || e < s) return "—";
  const ms = e - s;
  const sec = Math.floor(ms / 1000);
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  return `${hr}h ${min % 60}m`;
}

const SORT_INDICATORS: Record<SessionSortKey, Record<SessionSortDir, string>> = {
  started_at: { asc: "↑", desc: "↓" },
  agent_id: { asc: "↑", desc: "↓" },
  status: { asc: "↑", desc: "↓" },
};

export function SessionList({
  sessions,
  currentSessionId = null,
  variant = "sidebar",
  loading = false,
  onLoad,
  onRename,
  onDelete,
  onStop,
  className,
  headerLabel,
  headerAction,
}: SessionListProps) {
  const [search, setSearch] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // EP-0028-b: in the sidebar variant the sort order is fixed
  // (most recent first) — the UI controls are hidden anyway. We
  // still keep the state machinery for the expanded variant where
  // users can re-sort by agent or status.
  const [statusFilter, setStatusFilter] = useState<SessionStatusFilter>("all");
  const [sortKey, setSortKey] = useState<SessionSortKey>("started_at");
  const [sortDir, setSortDir] = useState<SessionSortDir>("desc");
  const [deleteTarget, setDeleteTarget] = useState<SessionSummary | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const editInputRef = useRef<HTMLInputElement>(null);

  const isSidebar = variant === "sidebar";
  const isExpanded = variant === "expanded";
  const header = headerLabel ?? (isSidebar ? "History" : "Sessions");

  const stats = useMemo(() => {
    let active = 0;
    let closed = 0;
    for (const s of sessions) {
      if (s.ended_at) closed += 1;
      else active += 1;
    }
    return { total: sessions.length, active, closed };
  }, [sessions]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    let list = sessions;
    if (q && !isSidebar) {
      // EP-0028-b: sidebar hides the search input (kept the
      // machinery for the other variants), so the filter only runs
      // for panel/expanded where the input is shown.
      list = list.filter(
        (s) =>
          (s.summary ?? "").toLowerCase().includes(q) ||
          s.session_id.toLowerCase().includes(q) ||
          (s.agent_id ?? "").toLowerCase().includes(q),
      );
    }
    if (statusFilter !== "all" && isExpanded) {
      list = list.filter((s) =>
        statusFilter === "closed" ? !!s.ended_at : !s.ended_at,
      );
    }
    // Effective sort: sidebar is always most-recent-first by
    // started_at. Expanded/panel respect the user's choice.
    const effSortKey: SessionSortKey = isSidebar ? "started_at" : sortKey;
    const effSortDir: SessionSortDir = isSidebar ? "desc" : sortDir;
    list = [...list].sort((a, b) => {
      let cmp = 0;
      if (effSortKey === "started_at") {
        cmp = (a.started_at ?? "").localeCompare(b.started_at ?? "");
      } else if (effSortKey === "agent_id") {
        cmp = (a.agent_id ?? "").localeCompare(b.agent_id ?? "");
      } else if (effSortKey === "status") {
        const aClosed = a.ended_at ? 1 : 0;
        const bClosed = b.ended_at ? 1 : 0;
        cmp = aClosed - bClosed;
      }
      return effSortDir === "asc" ? cmp : -cmp;
    });
    return list;
  }, [sessions, search, statusFilter, sortKey, sortDir, isSidebar]);

  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  const startRename = useCallback((s: SessionSummary) => {
    setEditingId(s.session_id);
    setEditValue(s.summary || s.session_id.slice(0, 12));
    setError(null);
  }, []);

  const cancelRename = useCallback(() => {
    setEditingId(null);
    setEditValue("");
  }, []);

  const commitRename = useCallback(
    async (s: SessionSummary) => {
      if (!onRename) {
        cancelRename();
        return;
      }
      const trimmed = editValue.trim();
      if (!trimmed) {
        cancelRename();
        return;
      }
      if (trimmed === (s.summary || "")) {
        cancelRename();
        return;
      }
      setBusy(`rename:${s.session_id}`);
      setError(null);
      try {
        await onRename(s.session_id, trimmed);
        cancelRename();
      } catch (e) {
        setError(`rename failed: ${(e as Error).message}`);
      } finally {
        setBusy(null);
      }
    },
    [editValue, onRename, cancelRename],
  );

  const requestDelete = useCallback(
    (s: SessionSummary) => {
      if (!onDelete) return;
      setDeleteTarget(s);
    },
    [onDelete],
  );

  const confirmDelete = useCallback(async () => {
    if (!deleteTarget || !onDelete) return;
    const target = deleteTarget;
    setDeleteTarget(null);
    setBusy(`delete:${target.session_id}`);
    setError(null);
    try {
      await onDelete(target.session_id);
    } catch (e) {
      setError(`delete failed: ${(e as Error).message}`);
    } finally {
      setBusy(null);
    }
  }, [deleteTarget, onDelete]);

  const handleStop = useCallback(
    async (s: SessionSummary) => {
      if (!onStop) return;
      setBusy(`stop:${s.session_id}`);
      setError(null);
      try {
        await onStop(s.session_id);
      } catch (e) {
        setError(`stop failed: ${(e as Error).message}`);
      } finally {
        setBusy(null);
      }
    },
    [busy, onStop],
  );

  const handleClick = useCallback(
    (s: SessionSummary) => {
      if (editingId === s.session_id) return;
      onLoad?.(s.session_id);
    },
    [editingId, onLoad],
  );

  const handleSort = useCallback(
    (key: SessionSortKey) => {
      if (sortKey === key) {
        setSortDir(sortDir === "asc" ? "desc" : "asc");
      } else {
        setSortKey(key);
        setSortDir("desc");
      }
    },
    [sortKey, sortDir],
  );

  const renderActions = (s: SessionSummary) => {
    if (!onRename && !onStop && !onDelete) return null;
    const editing = editingId === s.session_id;
    return (
      <div className="session-list__item-actions">
        {onRename && !editing && (
          <button
            className="session-list__item-edit-btn"
            onClick={(e) => {
              e.stopPropagation();
              startRename(s);
            }}
            title="Rename"
            aria-label="Rename session"
            data-testid={`session-list-rename-${s.session_id}`}
          >
            ✎
          </button>
        )}
        {onStop && !editing && !isClosed(s) && (
          <button
            className="session-list__item-stop"
            onClick={(e) => {
              e.stopPropagation();
              void handleStop(s);
            }}
            disabled={busy === `stop:${s.session_id}`}
            title="Stop session agent"
            aria-label="Stop session agent"
            data-testid={`session-list-stop-${s.session_id}`}
          >
            {busy === `stop:${s.session_id}` ? "…" : "■"}
          </button>
        )}
        {onDelete && !editing && (
          <button
            className={`session-list__item-delete${busy === `delete:${s.session_id}` ? " session-list__item-delete--busy" : ""}`}
            onClick={(e) => {
              e.stopPropagation();
              requestDelete(s);
            }}
            title="Delete session"
            aria-label="Delete session"
            data-testid={`session-list-delete-${s.session_id}`}
          >
            <IconTrash />
          </button>
        )}
      </div>
    );
  };

  const isActive = (s: SessionSummary) => s.session_id === currentSessionId;
  const isClosed = (s: SessionSummary) => !!s.ended_at;

  return (
    <div className={`session-list session-list--${variant}${className ? ` ${className}` : ""}`}>
      <div className="session-list__header">
        <span className="session-list__header-title">{header}</span>
        {isExpanded ? (
          <span className="session-list__stats" aria-label="Session stats">
            <span className="session-list__stat session-list__stat--total">
              {stats.total} total
            </span>
            <span className="session-list__stat session-list__stat--active">
              ● {stats.active} active
            </span>
            <span className="session-list__stat session-list__stat--closed">
              ○ {stats.closed} closed
            </span>
          </span>
        ) : (
          headerAction
        )}
      </div>

      {/* EP-0028-b: both sidebar and panel/expanded render the
          search box. Sidebar uses a compact layout. */}
      {true && (
        <div className="session-list__search">
          <IconSearch />
          <input
            ref={inputRef}
            type="text"
            placeholder={
              isExpanded
                ? "Search by summary, id, or agent…"
                : "Search sessions…"
            }
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            spellCheck={false}
            aria-label="Search sessions"
            data-testid="session-list-search"
          />
          {search && (
            <button
              type="button"
              className="session-list__search-clear"
              onClick={() => {
                setSearch("");
                inputRef.current?.focus();
              }}
              aria-label="Clear search"
              title="Clear"
            >
              ×
            </button>
          )}
        </div>
      )}

      <div className="session-list__filters" role="tablist" aria-label="Filter by status">
          {(["all", "active", "closed"] as const).map((f) => (
            <button
              key={f}
              type="button"
              role="tab"
              aria-selected={statusFilter === f}
              className={`session-list__filter-chip${statusFilter === f ? " session-list__filter-chip--active" : ""}`}
              onClick={() => setStatusFilter(f)}
              data-testid={`session-list-filter-${f}`}
            >
              {f === "all" ? "All" : f === "active" ? "● Active" : "○ Closed"}
            </button>
          ))}
        </div>

      {error && (
        <div className="session-list__error" role="alert">
          {error}
        </div>
      )}

      {loading && sessions.length === 0 ? (
        <div className="session-list__empty session-list__empty--loading">
          <div className="session-list__empty-icon">⏳</div>
          <div>Loading sessions…</div>
        </div>
      ) : sessions.length === 0 ? (
        <div className="session-list__empty">
          <div className="session-list__empty-icon">
            <IconClipboard />
          </div>
          <div className="session-list__empty-title">
            {isSidebar ? "No previous chats" : "No sessions yet"}
          </div>
          <div className="session-list__empty-subtitle">
            {isSidebar
              ? "Start a new chat to see history here"
              : "Sessions will appear here when you start chatting"}
          </div>
        </div>
      ) : filtered.length === 0 ? (
        <div className="session-list__empty">
          <div className="session-list__empty-icon">🔍</div>
          <div className="session-list__empty-subtitle">
            No matches
            {search && ` for "${search}"`}
            {statusFilter !== "all" && ` (${statusFilter})`}
          </div>
        </div>
      ) : (
        <>
{!isSidebar && isExpanded ? (
          <div className="session-list__sort-headers" aria-label="Sort by column">
            <button
              type="button"
              className={`session-list__sort-header${sortKey === "started_at" ? " session-list__sort-header--active" : ""}`}
              onClick={() => handleSort("started_at")}
              data-testid="session-list-sort-started_at"
            >
              When {sortKey === "started_at" && SORT_INDICATORS.started_at[sortDir]}
            </button>
            <button
              type="button"
              className={`session-list__sort-header${sortKey === "agent_id" ? " session-list__sort-header--active" : ""}`}
              onClick={() => handleSort("agent_id")}
              data-testid="session-list-sort-agent_id"
            >
              Agent {sortKey === "agent_id" && SORT_INDICATORS.agent_id[sortDir]}
            </button>
            <button
              type="button"
              className={`session-list__sort-header${sortKey === "status" ? " session-list__sort-header--active" : ""}`}
              onClick={() => handleSort("status")}
              data-testid="session-list-sort-status"
            >
              Status {sortKey === "status" && SORT_INDICATORS.status[sortDir]}
            </button>
            <div className="session-list__sort-header session-list__sort-header--spacer" />
          </div>
        ) : isSidebar ? (
          <div className="session-list__sort-headers session-list__sort-headers--sidebar" aria-label="Sort by column">
            <button
              type="button"
              className={`session-list__sort-header${sortKey === "started_at" ? " session-list__sort-header--active" : ""}`}
              onClick={() => handleSort("started_at")}
              data-testid="session-list-sort-started_at"
            >
              When {sortKey === "started_at" && SORT_INDICATORS.started_at[sortDir]}
            </button>
            <button
              type="button"
              className={`session-list__sort-header${sortKey === "agent_id" ? " session-list__sort-header--active" : ""}`}
              onClick={() => handleSort("agent_id")}
              data-testid="session-list-sort-agent_id"
            >
              Agent {sortKey === "agent_id" && SORT_INDICATORS.agent_id[sortDir]}
            </button>
            <button
              type="button"
              className={`session-list__sort-header${sortKey === "status" ? " session-list__sort-header--active" : ""}`}
              onClick={() => handleSort("status")}
              data-testid="session-list-sort-status"
            >
              Status {sortKey === "status" && SORT_INDICATORS.status[sortDir]}
            </button>
          </div>
        ) : null}
          <div className={`session-list__items ${isExpanded ? "session-list__items--expanded" : ""}`}>
            {filtered.map((s) => {
              const active = isActive(s);
              const closed = isClosed(s);
              const isEditing = editingId === s.session_id;
              const title = s.summary || (isSidebar ? s.session_id : s.session_id.slice(0, 8) + "…");
              // EP-0028-b: in the sidebar variant the title can
              // wrap to a second line so users get a preview of the
              // chat without expanding. In the panel/expanded
              // variants it stays single-line.
              const allowWrap = isSidebar && !!s.summary;

              return (
                <div
                  key={s.session_id}
                  className={`session-list__item${active ? " session-list__item--active" : ""}${closed ? " session-list__item--closed" : ""}${isExpanded ? " session-list__item--expanded" : ""}`}
                  onClick={() => handleClick(s)}
                  data-testid={`session-list-item-${s.session_id}`}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      handleClick(s);
                    }
                  }}
                >
                  {isEditing ? (
                    <input
                      ref={editInputRef}
                      className="session-list__item-edit"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void commitRename(s);
                        if (e.key === "Escape") cancelRename();
                      }}
                      onBlur={() => void commitRename(s)}
                      onClick={(e) => e.stopPropagation()}
                      spellCheck={false}
                      aria-label="Rename session"
                      data-testid="session-list-rename-input"
                    />
                  ) : (
                    <>
                      {isExpanded && (
                        <div
                          className={`session-list__item-status${closed ? " session-list__item-status--closed" : " session-list__item-status--active"}`}
                          aria-label={closed ? "closed" : "active"}
                          title={closed ? "Closed" : "Active"}
                        />
                      )}

                      <div className="session-list__item-body">
                        <div className="session-list__item-title-row">
                          <span
                            className={`session-list__item-title${allowWrap ? " session-list__item-title--wrap" : ""}`}
                            onDoubleClick={(e) => {
                              e.stopPropagation();
                              if (onRename) startRename(s);
                            }}
                            title={s.summary ?? s.session_id}
                          >
                            {title}
                          </span>
                          {!isExpanded && !isSidebar && (
                            <span
                              className={`session-list__status${closed ? " session-list__status--closed" : " session-list__status--active"}`}
                            >
                              {closed ? "closed" : "active"}
                            </span>
                          )}
                          {isSidebar && (
                            // EP-0028-b: the active indicator is a
                            // small dot for the sidebar variant —
                            // a status pill would dominate the row.
                            <span
                              className={`session-list__item-status session-list__item-status--inline${closed ? " session-list__item-status--closed" : " session-list__item-status--active"}`}
                              aria-label={closed ? "closed" : "active"}
                              title={closed ? "Closed" : "Active"}
                            />
                          )}
                        </div>
                        {/* EP-0028-b: sidebar lays out each metadata
                            chip on its own row with an icon prefix,
                            using the full sidebar width. Other
                            variants keep the inline dot-separated
                            meta row. */}
                        {isSidebar ? (
                          <div className="session-list__item-meta session-list__item-meta--stacked">
                            <span className="session-list__item-meta-row session-list__item-meta-row--agent">
                              <span className="session-list__item-meta-icon" aria-hidden="true">@</span>
                              <span className="session-list__item-agent">{s.agent_id}</span>
                            </span>
                            <span
                              className="session-list__item-meta-row session-list__item-meta-row--date"
                              title={formatDateLong(s.started_at)}
                            >
                              <span className="session-list__item-meta-icon" aria-hidden="true">▣</span>
                              <span className="session-list__item-date">
                                {formatDateShort(s.started_at)}
                              </span>
                            </span>
                            <span className="session-list__item-meta-row session-list__item-meta-row--duration">
                              <span className="session-list__item-meta-icon" aria-hidden="true">⏱</span>
                              <span className="session-list__item-duration">
                                {closed
                                  ? computeDuration(s.started_at, s.ended_at)
                                  : computeDuration(s.started_at, null)}
                              </span>
                            </span>
                          </div>
                        ) : (
                          <div className="session-list__item-meta">
                            {isExpanded && (
                              <span className="session-list__item-meta-item session-list__item-meta-agent">
                                {s.agent_id}
                              </span>
                            )}
                            {!isExpanded && (
                              <span className="session-list__item-agent">{s.agent_id}</span>
                            )}
                            {isExpanded && (
                              <span className="session-list__item-meta-dot">·</span>
                            )}
                            {!isExpanded && <span className="session-list__item-dot">·</span>}
                            <span
                              className="session-list__item-date"
                              title={formatDateLong(s.started_at)}
                            >
                              {formatDateShort(s.started_at)}
                            </span>
                            <span className="session-list__item-dot">·</span>
                            <span className="session-list__item-duration">
                              {closed
                                ? computeDuration(s.started_at, s.ended_at)
                                : computeDuration(s.started_at, null)}
                            </span>
                            {isExpanded && (
                              <>
                                <span className="session-list__item-meta-dot">·</span>
                                <span
                                  className="session-list__item-id"
                                  title={s.session_id}
                                >
                                  {s.session_id.slice(0, 8)}
                                </span>
                              </>
                            )}
                          </div>
                        )}
                        {isSidebar && renderActions(s)}
                      </div>

                      {!isSidebar && renderActions(s)}
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </>
      )}

      <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete session"
        message={
          <>
            Delete{" "}
            <strong>{deleteTarget?.summary || deleteTarget?.session_id}</strong>?
            This permanently removes its message history.
          </>
        }
        confirmLabel="Delete"
        destructive
        onConfirm={() => void confirmDelete()}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}
