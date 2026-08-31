import type { ReactNode } from "react";

export interface TableHeaderProps {
  children: ReactNode;
  className?: string;
}

/** Atomic <thead>. */
export function TableHeader({ children, className = "" }: TableHeaderProps) {
  return (
    <thead className={["table__header", className].filter(Boolean).join(" ")}>
      {children}
    </thead>
  );
}
