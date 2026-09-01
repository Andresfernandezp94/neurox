import type { ReactNode, HTMLAttributes } from "react";

export type RowJustify = "start" | "between" | "end" | "around";
export type RowAlign = "start" | "center" | "end" | "stretch";
export type RowGap = "none" | "sm" | "md" | "lg" | "xl";

export interface RowProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  justify?: RowJustify;
  align?: RowAlign;
  gap?: RowGap;
  wrap?: boolean;
  className?: string;
}

const justifyClass: Record<RowJustify, string> = {
  start: "row--start",
  between: "row--between",
  end: "row--end",
  around: "row--around",
};

const alignClass: Record<RowAlign, string> = {
  start: "row--align-start",
  center: "row--align-center",
  end: "row--align-end",
  stretch: "row--align-stretch",
};

const gapClass: Record<RowGap, string> = {
  none: "",
  sm: "row--gap-sm",
  md: "row--gap-md",
  lg: "row--gap-lg",
  xl: "row--gap-xl",
};

/**
 * Horizontal flex row.
 *
 * Use for headers, button bars, and any horizontal layout. Replaces
 * the legacy `.row` / `.row-between` utility classes plus the inline
 * `display: flex; gap: ...` patterns.
 */
export function Row({
  children,
  justify = "start",
  align = "center",
  gap = "md",
  wrap = false,
  className = "",
  ...rest
}: RowProps) {
  const classes = [
    "row",
    justifyClass[justify],
    alignClass[align],
    gapClass[gap],
    wrap ? "row--wrap" : "",
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
