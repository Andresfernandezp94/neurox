import type { HTMLAttributes, ReactNode } from "react";

export type StackGap = "none" | "sm" | "md" | "lg";
export type StackDirection = "column" | "row";

export interface StackProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  gap?: StackGap;
  direction?: StackDirection;
  align?: "start" | "center" | "end" | "stretch";
  className?: string;
}

const gapClass: Record<StackGap, string> = {
  none: "",
  sm: "stack--gap-sm",
  md: "stack--gap-md",
  lg: "stack--gap-lg",
};

const directionClass: Record<StackDirection, string> = {
  column: "",
  row: "stack--row",
};

const alignClass: Record<NonNullable<StackProps["align"]>, string> = {
  start: "stack--align-start",
  center: "stack--align-center",
  end: "stack--align-end",
  stretch: "stack--align-stretch",
};

/**
 * Vertical flex stack.
 *
 * Replaces inline styles like
 * `display: flex; flexDirection: column; gap: 0.75rem` (which appear
 * 5+ times across the admin panels).
 */
export function Stack({
  children,
  gap = "md",
  direction = "column",
  align,
  className = "",
  ...rest
}: StackProps) {
  const classes = [
    "stack",
    directionClass[direction],
    gapClass[gap],
    align ? alignClass[align] : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes} {...rest}>
      {children}
    </div>
  );
}
