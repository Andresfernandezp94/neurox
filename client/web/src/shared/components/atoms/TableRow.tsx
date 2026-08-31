import type { HTMLAttributes, ReactNode } from "react";

export interface TableRowProps extends HTMLAttributes<HTMLTableRowElement> {
  children: ReactNode;
  /** Visual emphasis (e.g. selected row). */
  selected?: boolean;
  /** Click handler — makes the row clickable (cursor + hover). */
  onClick?: () => void;
}

/** Atomic <tr>. */
export function TableRow({
  children,
  selected = false,
  onClick,
  className = "",
  ...rest
}: TableRowProps) {
  const classes = [
    "table__row",
    selected ? "table__row--selected" : "",
    onClick ? "table__row--clickable" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <tr className={classes} onClick={onClick} {...rest}>
      {children}
    </tr>
  );
}
