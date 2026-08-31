import type { ReactNode, ElementType, HTMLAttributes } from "react";

export type CardGap = "none" | "sm" | "md" | "lg";

export interface CardProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  className?: string;
  /** Polymorphic root element. Default: <div>. */
  as?: ElementType;
  /** Internal gap between children. Default: md (0.5rem). */
  gap?: CardGap;
}

const gapClass: Record<CardGap, string> = {
  none: "",
  sm: "stack--gap-sm",
  md: "stack stack--gap-md",
  lg: "stack stack--gap-lg",
};

/**
 * Atomic card container.
 *
 * Compound API: <Card> + <Card.Title> + <Card.Body> + <Card.Actions>.
 * Maps to the legacy `.card` utility class. Renders as a vertical
 * flex stack by default (gap=md) to match the inline layout used
 * before atomic design was introduced.
 */
export function Card({
  children,
  className = "",
  as: Tag = "div",
  gap = "md",
}: CardProps) {
  const classes = ["card", gapClass[gap], className]
    .filter(Boolean)
    .join(" ");
  return <Tag className={classes}>{children}</Tag>;
}

interface SubProps {
  children: ReactNode;
  className?: string;
}

function CardTitle({ children, className = "" }: SubProps) {
  return (
    <div className={["card__title", className].filter(Boolean).join(" ")}>
      {children}
    </div>
  );
}

function CardBody({ children, className = "" }: SubProps) {
  return (
    <div className={["card__body", className].filter(Boolean).join(" ")}>
      {children}
    </div>
  );
}

function CardActions({ children, className = "" }: SubProps) {
  return (
    <div
      className={["card__actions", className].filter(Boolean).join(" ")}
    >
      {children}
    </div>
  );
}

Card.Title = CardTitle;
Card.Body = CardBody;
Card.Actions = CardActions;
