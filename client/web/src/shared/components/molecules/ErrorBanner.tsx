import type { ReactNode } from "react";

export type ErrorBannerVariant = "error" | "warn" | "info";

export interface ErrorBannerProps {
  children: ReactNode;
  variant?: ErrorBannerVariant;
  onDismiss?: () => void;
  className?: string;
}

const variantClass: Record<ErrorBannerVariant, string> = {
  error: "error-banner",
  warn: "error-banner error-banner--warn",
  info: "error-banner error-banner--info",
};

/**
 * Inline error/warn/info banner.
 *
 * Replaces the legacy `.error-banner` utility class. When `onDismiss`
 * is provided, a close button is rendered.
 */
export function ErrorBanner({
  children,
  variant = "error",
  onDismiss,
  className = "",
}: ErrorBannerProps) {
  return (
    <div
      className={[variantClass[variant], className]
        .filter(Boolean)
        .join(" ")}
      role={variant === "error" ? "alert" : "status"}
    >
      <span className="error-banner__content">{children}</span>
      {onDismiss && (
        <button
          type="button"
          className="error-banner__dismiss"
          onClick={onDismiss}
          aria-label="Dismiss"
        >
          ×
        </button>
      )}
    </div>
  );
}
