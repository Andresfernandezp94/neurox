import type { ReactNode } from "react";

export type StatVariant = "default" | "total" | "active" | "closed";

export interface StatPairProps {
  label: ReactNode;
  value: ReactNode;
  variant?: StatVariant;
  className?: string;
}

const variantClass: Record<StatVariant, string> = {
  default: "stat-pair",
  total: "stat-pair stat-pair--total",
  active: "stat-pair stat-pair--active",
  closed: "stat-pair stat-pair--closed",
};

/**
 * Atomic label + value pair used in panel headers.
 *
 * Replaces the 3 variants of `.session-list__stat` used in SessionList
 * (total/active/closed).
 */
export function StatPair({
  label,
  value,
  variant = "default",
  className = "",
}: StatPairProps) {
  return (
    <div
      className={[variantClass[variant], className]
        .filter(Boolean)
        .join(" ")}
    >
      <span className="stat-pair__label">{label}</span>
      <span className="stat-pair__value">{value}</span>
    </div>
  );
}
