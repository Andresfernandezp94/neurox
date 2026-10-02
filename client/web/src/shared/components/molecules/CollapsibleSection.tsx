// CollapsibleSection — a controlled <details>-based molecule.
//
// EP-0024-UX: wraps a section with a clickable header that can
// collapse its body. Renders a chevron that rotates on open state.
// Uses controlled state so the open/closed state can be reset
// from outside (e.g. when the draft is loaded fresh). The user's
// choice persists across re-renders of the parent unless
// `defaultOpen` changes.
//
// Usage:
//   <CollapsibleSection title="General" defaultOpen>
//     ...
//   </CollapsibleSection>

import { useState, type ReactNode } from "react";

export interface CollapsibleSectionProps {
  /** Title rendered inside the clickable summary. */
  title: ReactNode;
  /** Optional small text after the title (count, hint, etc.). */
  badge?: ReactNode;
  /**
   * Classes for the badge wrapper, e.g. `collapsible__badge--accent`.
   *
   * The wrapper element already carries `collapsible__badge`, so pass the
   * CONTENT here (usually the count) and pick the variant with this prop.
   * Passing a `<span className="collapsible__badge">` as `badge` instead
   * nests a badge inside the wrapper: two borders, two fills and doubled
   * horizontal padding, and the accent variant ends up framed by a neutral
   * pill.
   */
  badgeClassName?: string;
  /** Optional subtitle / hint text under the title. */
  hint?: ReactNode;
  /** Initial open state. Defaults to false. */
  defaultOpen?: boolean;
  /** Body content, rendered below the summary when open. */
  children: ReactNode;
  /** Class applied to the outer <details>. */
  className?: string;
  /** data-testid forwarded to the <details>. */
  "data-testid"?: string;
}

export function CollapsibleSection({
  title,
  badge,
  badgeClassName,
  hint,
  defaultOpen = false,
  children,
  className = "",
  "data-testid": testId,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <details
      className={["collapsible", className].filter(Boolean).join(" ")}
      open={open}
      data-testid={testId}
      onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)}
    >
      <summary className="collapsible__summary">
        <span className="collapsible__chevron" aria-hidden="true">
          ▸
        </span>
        <span className="collapsible__title">{title}</span>
        {badge != null && (
          <span
            className={["collapsible__badge", badgeClassName]
              .filter(Boolean)
              .join(" ")}
          >
            {badge}
          </span>
        )}
        {hint != null && <span className="collapsible__hint">{hint}</span>}
      </summary>
      <div className="collapsible__body">{children}</div>
    </details>
  );
}