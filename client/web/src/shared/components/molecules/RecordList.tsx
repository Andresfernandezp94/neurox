// RecordList — generic list of records (logs, sessions, events). Each
// record is a row with: leading indicator (status dot), header (title +
// subtitle), trailing meta (timestamp / status), and a fields row.
//
// When `collapsible`, the body (fields) is hidden by default and the
// title acts as a toggle. Per-row state is internal — controlled via
// the `expandedIds` Set from the parent if you need to lift state.
// Use when the data is a sequence of timestamped events where each row
// has heterogeneous fields. For tabular data with multiple columns,
// use <DataTable<T>> instead.

import { useState, type ReactNode } from "react";
import type { BadgeVariant } from "../atoms/Badge";

export interface RecordField {
  /** Optional label rendered above the value. */
  label?: string;
  /** The value to render (text, JSX, etc.). */
  value: ReactNode;
  /** Optional width (e.g. "12rem"). */
  width?: string;
  /** Highlight color for the value. */
  variant?: BadgeVariant | "default";
}

export interface RecordItem {
  /** Stable id (used as React key). */
  id: string;
  /** Header line (e.g. "session started", "tool call"). */
  title: ReactNode;
  /** Optional subtitle under the title. */
  subtitle?: ReactNode;
  /** Optional trailing meta (right-aligned, e.g. timestamp). */
  meta?: ReactNode;
  /** Optional status indicator (left dot). */
  status?: { variant: BadgeVariant; label?: string };
  /** Optional fields row (key-value pairs). */
  fields?: RecordField[];
  /** Optional click handler. */
  onClick?: () => void;
}

export interface RecordListProps {
  items: RecordItem[];
  /** Empty state. */
  empty?: ReactNode;
  /** Compact (denser) rows. */
  compact?: boolean;
  /** Sticky header on scroll (for the title row, not the items). */
  stickyHeader?: boolean;
  /** Clickable rows get hover state. */
  interactive?: boolean;
  /**
   * Collapse the body (fields) by default. The title becomes a toggle
   * the user can click to expand/collapse. Useful for log streams
   * where each row carries a heavy JSON payload.
   */
  collapsible?: boolean;
  /** Ids of rows that should start expanded. Only applies when `collapsible`. */
  expandedIds?: string[];
  /** Called when a row's expanded state changes. */
  onExpandedChange?: (id: string, expanded: boolean) => void;
}

/**
 * Vertical list of records. Each record is a row with:
 *   [status dot]  [title + subtitle]                    [meta]
 *                 [fields row if any]
 */
export function RecordList({
  items,
  empty,
  compact = false,
  interactive = false,
  collapsible = false,
  expandedIds,
  onExpandedChange,
}: RecordListProps) {
  const [internalExpanded, setInternalExpanded] = useState<Set<string>>(
    () => new Set(expandedIds ?? []),
  );

  if (items.length === 0 && empty !== undefined) {
    return <>{empty}</>;
  }

  const toggle = (id: string) => {
    setInternalExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
        onExpandedChange?.(id, false);
      } else {
        next.add(id);
        onExpandedChange?.(id, true);
      }
      return next;
    });
  };

  return (
    <ul
      className={[
        "record-list",
        compact ? "record-list--compact" : "",
        collapsible ? "record-list--collapsible" : "",
      ].filter(Boolean).join(" ")}
    >
      {items.map((item) => (
        <RecordListItem
          key={item.id}
          item={item}
          interactive={interactive || !!item.onClick}
          collapsible={collapsible}
          expanded={internalExpanded.has(item.id)}
          onToggle={() => toggle(item.id)}
        />
      ))}
    </ul>
  );
}

function RecordListItem({
  item,
  interactive,
  collapsible,
  expanded,
  onToggle,
}: {
  item: RecordItem;
  interactive: boolean;
  collapsible: boolean;
  expanded: boolean;
  onToggle: () => void;
}) {
  const handleTitleClick = (e: React.MouseEvent) => {
    if (collapsible) {
      e.preventDefault();
      e.stopPropagation();
      onToggle();
    }
  };

  const handleRowClick = () => {
    if (item.onClick) item.onClick();
  };

  return (
    <li
      className={[
        "record-list__item",
        interactive ? "record-list__item--interactive" : "",
        collapsible ? "record-list__item--collapsible" : "",
        collapsed(collapsible, expanded) ? "record-list__item--collapsed" : "",
      ].filter(Boolean).join(" ")}
    >
      <div
        className={[
          "record-list__row",
          interactive || collapsible ? "record-list__row--clickable" : "",
        ].filter(Boolean).join(" ")}
        onClick={handleRowClick}
        role={collapsible ? "button" : undefined}
        aria-expanded={collapsible ? expanded : undefined}
        tabIndex={collapsible ? 0 : undefined}
        onKeyDown={
          collapsible
            ? (e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onToggle();
                }
              }
            : undefined
        }
      >
        {item.status && (
          <span
            className={`record-list__status record-list__status--${item.status.variant}`}
            aria-label={item.status.label ?? item.status.variant}
          />
        )}
        <div className="record-list__body">
          <div
            className={[
              "record-list__header",
              collapsible ? "record-list__header--toggleable" : "",
            ].filter(Boolean).join(" ")}
            onClick={collapsible ? handleTitleClick : undefined}
          >
            <span className="record-list__title">{item.title}</span>
            {item.subtitle && (
              <span className="record-list__subtitle">{item.subtitle}</span>
            )}
          </div>
          {item.fields && item.fields.length > 0 && (
            <dl
              className={[
                "record-list__fields",
                collapsed(collapsible, expanded) ? "record-list__fields--hidden" : "",
              ].filter(Boolean).join(" ")}
            >
              {item.fields.map((f, fIndex) => (
                <div key={fIndex} className="record-list__field">
                  {f.label && (
                    <dt className="record-list__field-label">{f.label}</dt>
                  )}
                  <dd
                    className={[
                      "record-list__field-value",
                      f.variant && f.variant !== "default"
                        ? `record-list__field-value--${f.variant}`
                        : "",
                    ].filter(Boolean).join(" ")}
                    style={f.width ? { width: f.width } : undefined}
                  >
                    {f.value}
                  </dd>
                </div>
              ))}
            </dl>
          )}
        </div>
        {item.meta && (
          <div className="record-list__meta">{item.meta}</div>
        )}
      </div>
    </li>
  );
}

function collapsed(collapsible: boolean, expanded: boolean): boolean {
  return collapsible && !expanded;
}
