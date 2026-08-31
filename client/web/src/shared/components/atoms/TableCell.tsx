import type { TdHTMLAttributes } from "react";
import type { TableAlign } from "./Table";

export interface TableCellProps extends TdHTMLAttributes<HTMLTableCellElement> {
  align?: TableAlign;
  /** Tighter padding override. */
  compact?: boolean;
}

/** Atomic <td>. */
export function TableCell({
  align = "left",
  compact = false,
  className = "",
  children,
  ...rest
}: TableCellProps) {
  const classes = [
    "table__cell",
    `table__cell--align-${align}`,
    compact ? "table__cell--compact" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <td className={classes} {...rest}>
      {children}
    </td>
  );
}
