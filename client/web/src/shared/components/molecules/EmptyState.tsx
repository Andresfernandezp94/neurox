import type { ReactNode } from "react";
import type { IconName } from "../atoms/Icon";

export interface EmptyStateProps {
  children: ReactNode;
  icon?: IconName;
  className?: string;
}

/**
 * Empty-state placeholder.
 *
 * Compound API: <EmptyState> + optional <EmptyState.Icon>,
 * <EmptyState.Title>, <EmptyState.Hint>.
 */
export function EmptyState({
  children,
  className = "",
}: EmptyStateProps) {
  return (
    <div className={["empty", className].filter(Boolean).join(" ")}>
      {children}
    </div>
  );
}

interface SubProps {
  children: ReactNode;
  className?: string;
}

function EmptyIcon({
  children,
  className = "",
}: SubProps) {
  return (
    <div className={["empty__icon", className].filter(Boolean).join(" ")}>
      {children}
    </div>
  );
}

function EmptyTitle({ children, className = "" }: SubProps) {
  return (
    <div className={["empty__title", className].filter(Boolean).join(" ")}>
      {children}
    </div>
  );
}

function EmptyHint({ children, className = "" }: SubProps) {
  return (
    <div className={["empty__hint", className].filter(Boolean).join(" ")}>
      {children}
    </div>
  );
}

EmptyState.Icon = EmptyIcon;
EmptyState.Title = EmptyTitle;
EmptyState.Hint = EmptyHint;
EmptyState.defaultIconName = "IconInbox";
