import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  /** Optional custom fallback label. */
  label?: string;
}

interface State {
  error: Error | null;
}

/**
 * Minimal Error Boundary. Catches render-time errors and displays the
 * message + stack instead of a blank screen. Used to surface crashes
 * in large experimental panels (e.g. StorageTab) so the operator can
 * report them without opening DevTools.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: { componentStack: string }) {
    // eslint-disable-next-line no-console
    console.error("[ErrorBoundary]", error, info);
  }

  render() {
    const { error } = this.state;
    if (error === null) return this.props.children;
    const { label = "panel" } = this.props;
    return (
      <div className="error-boundary" data-testid="error-boundary">
        <h3 className="error-boundary__title">{label} crashed</h3>
        <p className="error-boundary__message">{error.message}</p>
        <details className="error-boundary__details">
          <summary>Stack trace</summary>
          <pre className="error-boundary__stack">{error.stack ?? "(no stack)"}</pre>
        </details>
      </div>
    );
  }
}
