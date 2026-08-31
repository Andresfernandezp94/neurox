import type { ReactNode } from "react";

export interface TableBodyProps {
  children: ReactNode;
  className?: string;
}

/** Atomic <tbody>. */
export function TableBody({ children, className = "" }: TableBodyProps) {
  return (
    <tbody className={["table__body", className].filter(Boolean).join(" ")}>
      {children}
    </tbody>
  );
}
