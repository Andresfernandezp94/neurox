import type { ThHTMLAttributes } from "react";
import type { TableAlign } from "./Table";

export interface TableHeadProps extends ThHTMLAttributes<HTMLTableCellElement> {
  align?: TableAlign;
  /** Visual width (CSS value). */
  width?: string | number;
}

/** Atomic <th>. */
export function TableHead({
  align = "left",
  width,
  className = "",
  style,
  children,
  ...rest
}: TableHeadProps) {
  const classes = [
    "table__head",
    `table__head--align-${align}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  const inlineStyle = {
    ...style,
    width: typeof width === "number" ? `${width}px` : width,
  };

  return (
    <th className={classes} style={inlineStyle} {...rest}>
      {children}
    </th>
  );
}
