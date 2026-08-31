import type { InputHTMLAttributes, ReactNode } from "react";
import { forwardRef } from "react";

export type InputSize = "sm" | "md";

export interface InputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "size"> {
  size?: InputSize;
  invalid?: boolean;
  className?: string;
  /** Optional adornment rendered inside the input on the right side. */
  rightAdornment?: ReactNode;
}

const sizeClass: Record<InputSize, string> = {
  sm: "input--sm",
  md: "",
};

/**
 * Atomic text input.
 *
 * Forwards a ref to the underlying <input> element so callers can
 * focus, blur, or select imperatively.
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  {
    size = "md",
    invalid = false,
    rightAdornment,
    className = "",
    type = "text",
    ...rest
  },
  ref,
) {
  const classes = [
    "input",
    sizeClass[size],
    invalid ? "input--invalid" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={`input-wrapper ${invalid ? "input-wrapper--invalid" : ""}`}>
      <input ref={ref} type={type} className={classes} {...rest} />
      {rightAdornment && (
        <span className="input-wrapper__adornment">{rightAdornment}</span>
      )}
    </div>
  );
});
