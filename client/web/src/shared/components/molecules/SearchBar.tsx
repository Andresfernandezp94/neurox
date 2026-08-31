import type { ChangeEvent } from "react";
import { Icon, type IconName } from "../atoms/Icon";

export interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  icon?: IconName;
  onClear?: () => void;
  className?: string;
  ariaLabel?: string;
}

/**
 * Atomic search input with optional clear button.
 *
 * Replaces the legacy `.session-list__search` and `.chat-history__search`
 * BEM-scoped patterns that were duplicated across 2 organisms.
 */
export function SearchBar({
  value,
  onChange,
  placeholder = "Search…",
  icon = "IconSearch",
  onClear,
  className = "",
  ariaLabel = "Search",
}: SearchBarProps) {
  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    onChange(e.target.value);
  };

  return (
    <div className={["search-bar", className].filter(Boolean).join(" ")}>
      <span className="search-bar__icon">
        <Icon name={icon} aria-hidden="true" />
      </span>
      <input
        type="search"
        className="search-bar__input"
        value={value}
        onChange={handleChange}
        placeholder={placeholder}
        aria-label={ariaLabel}
      />
      {value && onClear && (
        <button
          type="button"
          className="search-bar__clear"
          onClick={onClear}
          aria-label="Clear search"
        >
          ×
        </button>
      )}
    </div>
  );
}
