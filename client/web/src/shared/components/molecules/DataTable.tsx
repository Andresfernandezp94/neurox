// DataTable<T> — typed table that takes a column config. Builds on the
// Table atoms so styling stays consistent across the app.

import { Table, type TableAlign } from "../atoms/Table";
import { TableHeader } from "../atoms/TableHeader";
import { TableBody } from "../atoms/TableBody";
import { TableRow } from "../atoms/TableRow";
import { TableHead } from "../atoms/TableHead";
import { TableCell } from "../atoms/TableCell";
import { EmptyState } from "./EmptyState";

export interface Column<T> {
  /** Stable id (used as key + for sorting). */
  id: string;
  /** Header cell content. */
  header: React.ReactNode;
  /** Cell renderer for a row. */
  cell: (row: T, index: number) => React.ReactNode;
  /** CSS width. */
  width?: string | number;
  align?: TableAlign;
  /** Additional className applied to both head and cells. */
  className?: string;
}

export interface DataTableProps<T> {
  data: T[];
  columns: Column<T>[];
  /** Field or function to extract a stable key from each row. */
  keyField: keyof T | ((row: T) => string);
  /** Click handler — makes rows clickable. */
  onRowClick?: (row: T) => void;
  /** Predicate to mark rows as selected. */
  isSelected?: (row: T) => boolean;
  /** Custom empty state. */
  empty?: React.ReactNode;
  /** Sticky header on scroll. */
  stickyHeader?: boolean;
  /** Compact (denser) padding. */
  compact?: boolean;
  /** ARIA label for the table. */
  "aria-label"?: string;
}

/**
 * Generic typed table.
 *
 * @example
 *   <DataTable<Service>
 *     data={services}
 *     keyField="id"
 *     columns={[
 *       { id: "name", header: "Name", cell: (s) => s.name },
 *       { id: "status", header: "Status", cell: (s) => <Badge>{s.status}</Badge> },
 *     ]}
 *   />
 */
export function DataTable<T>({
  data,
  columns,
  keyField,
  onRowClick,
  isSelected,
  empty,
  stickyHeader = false,
  compact = false,
  ...rest
}: DataTableProps<T>) {
  const getKey = (row: T, index: number): string => {
    if (typeof keyField === "function") {
      return keyField(row);
    }
    return String(row[keyField] ?? index);
  };

  if (data.length === 0 && empty !== undefined) {
    return <>{empty}</>;
  }

  return (
    <Table
      stickyHeader={stickyHeader}
      compact={compact}
      aria-label={rest["aria-label"]}
    >
      <TableHeader>
        <TableRow>
          {columns.map((c) => (
            <TableHead
              key={c.id}
              align={c.align}
              width={c.width}
              className={c.className}
            >
              {c.header}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.length === 0 ? (
          <TableRow>
            <TableCell colSpan={columns.length}>
              <EmptyState>
                <EmptyState.Title>No rows</EmptyState.Title>
              </EmptyState>
            </TableCell>
          </TableRow>
        ) : (
          data.map((row, i) => (
            <TableRow
              key={getKey(row, i)}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
              selected={isSelected?.(row) ?? false}
            >
              {columns.map((c) => (
                <TableCell
                  key={c.id}
                  align={c.align}
                  className={c.className}
                >
                  {c.cell(row, i)}
                </TableCell>
              ))}
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  );
}
