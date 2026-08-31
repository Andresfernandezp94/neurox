// ErrorBoundary — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Mensaje en español + usa useI18n
// - Botón "Reintentar" (re-load) en vez de "Try again"

import { Component, type ReactNode } from "react";
import { useI18n } from "../hooks/useI18n";
import { Card } from "./molecules/Card";
import { Button } from "./atoms/Button";

interface State {
  error: Error | null;
}

interface Props {
  children: ReactNode;
}

export class ErrorBoundaryRaw extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    if (this.state.error) {
      return <ErrorFallback error={this.state.error} onReset={() => this.setState({ error: null })} />;
    }
    return this.props.children;
  }
}

function ErrorFallback({ error, onReset }: { error: Error; onReset: () => void }) {
  // useI18n can't be called inside a class component, so we wrap
  // the JSX in this function component.
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const { t } = useI18n();
  return (
    <Card className="error-boundary__card">
      <h2 className="strong error-boundary__title">
        {t("error.boundary.title")}
      </h2>
      <p className="muted error-boundary__message">
        {t("error.boundary.message")}
      </p>
      {error.message && (
        <pre className="code error-boundary__trace">
          {error.message}
        </pre>
      )}
      <Button variant="primary" onClick={onReset}>
        Reintentar
      </Button>
    </Card>
  );
}

export function ErrorBoundary({ children }: Props) {
  return <ErrorBoundaryRaw>{children}</ErrorBoundaryRaw>;
}