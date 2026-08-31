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
        {badge != null && <span className="collapsible__badge">{badge}</span>}
        {hint != null && <span className="collapsible__hint">{hint}</span>}
      </summary>
      <div className="collapsible__body">{children}</div>
    </details>
  );
}