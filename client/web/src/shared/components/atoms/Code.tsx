import type { HTMLAttributes, ReactNode } from "react";

export interface CodeProps
  extends Omit<HTMLAttributes<HTMLElement>, "className"> {
  children: ReactNode;
  className?: string;
  /** Render as a different tag (default: <code>). */
  as?: "code" | "span" | "kbd" | "samp";
}

/**
 * Atomic monospace inline text.
 *
 * Maps to the legacy `.code` utility class. Render as <code> by default
 * since most usages are technical terms or paths; pass `as="span"` for
 * non-semantic monospace text.
 */
export function Code({
  children,
  className = "",
  as = "code",
  ...rest
}: CodeProps) {
  const classes = ["text-mono", className].filter(Boolean).join(" ");
  const Tag = as;
  return (
    <Tag className={classes} {...rest}>
      {children}
    </Tag>
  );
}
