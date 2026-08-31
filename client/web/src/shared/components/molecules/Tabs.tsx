// Tabs — componente reutilizable de tabs con look retro CLI.
// Consistente con el sidebar: monospace, accent en active, sin
// borders redondeados, transición con steps(2, end) para feel pixelado.

import type { ReactNode } from "react";

export interface TabsItem<T extends string> {
  id: T;
  label: string;
  icon?: ReactNode;
}

export interface TabsProps<T extends string> {
  items: Array<TabsItem<T>>;
  active: T;
  onChange: (id: T) => void;
  /** Prefix del data-testid (ej: "status-tab" → "status-tab-overview"). */
  testIdPrefix?: string;
  /** Label accesible para el tablist. */
  ariaLabel?: string;
}

export function Tabs<T extends string>({
  items,
  active,
  onChange,
  testIdPrefix,
  ariaLabel,
}: TabsProps<T>) {
  return (
    <div className="tabs" role="tablist" aria-label={ariaLabel}>
      {items.map((item) => {
        const isActive = item.id === active;
        return (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={isActive}
            className={`tabs__tab ${isActive ? "is-active" : ""}`}
            onClick={() => onChange(item.id)}
            data-testid={testIdPrefix ? `${testIdPrefix}-${item.id}` : undefined}
          >
            {item.icon && <span className="tabs__icon">{item.icon}</span>}
            <span className="tabs__label">{item.label}</span>
          </button>
        );
      })}
    </div>
  );
}
