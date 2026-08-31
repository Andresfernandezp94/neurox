import type { ReactNode, ElementType } from "react";

export interface PanelProps {
  title?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  as?: ElementType;
}

/**
 * Panel = Card variant with mandatory header.
 *
 * Use when the panel needs a clear title and optional action row at the
 * top. Replaces the legacy `.panel` + `.panel-header` utility classes.
 */
export function Panel({
  title,
  actions,
  children,
  className = "",
  as: Tag = "section",
}: PanelProps) {
  return (
    <Tag className={["panel", className].filter(Boolean).join(" ")}>
      {(title || actions) && (
        <header className="panel__header">
          {title && <div className="panel__title">{title}</div>}
          {actions && <div className="panel__actions">{actions}</div>}
        </header>
      )}
      <div className="panel__body">{children}</div>
    </Tag>
  );
}
