import type { ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "./Icon";

export type IconButtonVariant = "default" | "danger" | "ghost";
export type IconButtonSize = "sm" | "md";

export interface IconButtonProps
  extends Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    "className" | "children" | "aria-label"
  > {
  icon: IconName;
  /** Required for screen readers — icon buttons have no visible text. */
  "aria-label": string;
  variant?: IconButtonVariant;
  size?: IconButtonSize;
  className?: string;
}

const variantClass: Record<IconButtonVariant, string> = {
  default: "icon-btn",
  danger: "icon-btn icon-btn--danger",
  ghost: "icon-btn icon-btn--ghost",
};

const sizeClass: Record<IconButtonSize, string> = {
  sm: "icon-btn--sm",
  md: "",
};

/**
 * Atomic icon-only button.
 *
 * Requires `aria-label` — icons alone are not accessible. Use for
 * compact actions like edit/delete in toolbars.
 */
export function IconButton({
  icon,
  variant = "default",
  size = "md",
  type = "button",
  className = "",
  ...rest
}: IconButtonProps) {
  const classes = [
    variantClass[variant],
    sizeClass[size],
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button type={type} className={classes} {...rest}>
      <Icon name={icon} aria-hidden="true" />
    </button>
  );
}
