import type { LabelHTMLAttributes, ReactNode } from "react";

export interface LabelProps
  extends Omit<LabelHTMLAttributes<HTMLLabelElement>, "className"> {
  children: ReactNode;
  required?: boolean;
  className?: string;
}

/**
 * Atomic <label> with the `.label-text` modifier.
 *
 * Renders a semantic <label htmlFor="..."> that associates with a form
 * control. Use `required` to append a visual indicator.
 */
export function Label({
  children,
  required = false,
  className = "",
  ...rest
}: LabelProps) {
  return (
    <label
      className={["label-text", required ? "label-text--required" : "", className]
        .filter(Boolean)
        .join(" ")}
      {...rest}
    >
      {children}
      {required && <span aria-hidden="true"> *</span>}
    </label>
  );
}
