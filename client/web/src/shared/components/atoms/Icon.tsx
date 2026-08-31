import type { CSSProperties } from "react";
import * as Icons from "../Icons";

// Exclude icons that require props (e.g., IconMode needs a `mode` prop).
// These can't be used through the generic <Icon name=...> wrapper.
type ExcludedIcons = "IconMode";
export type IconName = Exclude<keyof typeof Icons, ExcludedIcons>;

export type IconSize = "xs" | "sm" | "md" | "lg";

const sizeMap: Record<IconSize, string> = {
  xs: "0.75rem",
  sm: "0.875rem",
  md: "1rem",
  lg: "1.25rem",
};

export interface IconProps {
  name: IconName;
  size?: IconSize;
  color?: string;
  className?: string;
  style?: CSSProperties;
  /** Accessible label. If omitted, the icon is treated as decorative. */
  title?: string;
}

/**
 * Atomic wrapper around the named SVG icons in `./Icons.tsx`.
 *
 * Replaces ad-hoc `<IconFoo />` imports throughout the codebase.
 * Supports sizing via the `.icon` utility classes and a `color` token.
 */
export function Icon({
  name,
  size = "md",
  color,
  className = "",
  style,
  title,
}: IconProps) {
  const Component = Icons[name];
  const sizeStyle: CSSProperties = {
    width: sizeMap[size],
    height: sizeMap[size],
    ...(color ? { color } : {}),
    ...style,
  };

  const wrapperProps = title
    ? { role: "img" as const, "aria-label": title }
    : { "aria-hidden": true as const };

  return (
    <span
      className={`icon ${className}`}
      style={sizeStyle}
      {...wrapperProps}
    >
      <Component />
    </span>
  );
}
