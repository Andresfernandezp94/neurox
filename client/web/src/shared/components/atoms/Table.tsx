export type TableAlign = "left" | "center" | "right";

export interface TableProps extends React.TableHTMLAttributes<HTMLTableElement> {
  /** Sticky header on scroll. Default: false. */
  stickyHeader?: boolean;
  /** Compact (denser) row padding. Default: false. */
  compact?: boolean;
}

/**
 * Atomic <table> wrapper.
 *
 * Use `<Table>` + `<TableHeader>` + `<TableBody>` + `<TableRow>` +
 * `<TableHead>` + `<TableCell>` for full control, or `<DataTable<T>>`
 * for a typed, column-config-driven table.
 */
export function Table({
  stickyHeader = false,
  compact = false,
  className = "",
  children,
  ...rest
}: TableProps) {
  const classes = [
    "table",
    stickyHeader ? "table--sticky" : "",
    compact ? "table--compact" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <table className={classes} {...rest}>
      {children}
    </table>
  );
}
