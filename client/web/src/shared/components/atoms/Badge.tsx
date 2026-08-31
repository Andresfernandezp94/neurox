import type { HTMLAttributes, ReactNode } from "react";

export type BadgeVariant =
  | "neutral"
  | "success"
  | "warn"
  | "danger"
  | "info";

export interface BadgeProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "className"> {
  variant?: BadgeVariant;
  /** Render an animated status dot before the children. */
  dot?: boolean;
  children: ReactNode;
  className?: string;
}

const variantClass: Record<BadgeVariant, string> = {
  neutral: "badge",
  success: "badge badge--success",
  warn: "badge badge--warn",
  danger: "badge badge--danger",
  info: "badge badge--info",
};

/**
 * Atomic badge / status pill.
 *
 * Use `dot` to render the animated status indicator. Maps to the legacy
 * `.status-badge--*` classes that today mix BEM and non-BEM naming.
 */
export function Badge({
  variant = "neutral",
  dot = false,
  children,
  className = "",
  ...rest
}: BadgeProps) {
  const classes = [variantClass[variant], className]
    .filter(Boolean)
    .join(" ");

  return (
    <span className={classes} {...rest}>
      {dot && <span className="badge__dot" aria-hidden="true" />}
      {children}
    </span>
  );
}
