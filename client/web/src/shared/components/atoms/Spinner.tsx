export type SpinnerSize = "sm" | "md" | "lg";

export interface SpinnerProps {
  size?: SpinnerSize;
  /** Accessible label. Required for screen readers. */
  label?: string;
  className?: string;
}

/**
 * Atomic CSS-only spinner (rotation animation defined in atoms.css).
 *
 * Use `label` for a11y — otherwise the spinner is purely decorative.
 */
export function Spinner({
  size = "md",
  label = "Loading",
  className = "",
}: SpinnerProps) {
  const classes = ["spinner", `spinner--${size}`, className]
    .filter(Boolean)
    .join(" ");

  return (
    <span
      className={classes}
      role="status"
      aria-label={label}
    />
  );
}
