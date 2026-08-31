// Tests para ErrorBoundary.
// EP-0002-01 Task 5.

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ErrorBoundary } from "./ErrorBoundary";

function Bomb({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) throw new Error("test boom");
  return <div>safe content</div>;
}

// Suppress noisy console.error from React's error boundary logging
const origError = console.error;
beforeAll(() => {
  console.error = vi.fn();
});
afterAll(() => {
  console.error = origError;
});

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow={false} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("safe content")).toBeDefined();
  });

  it("renders the fallback when a child throws", () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Algo salió mal")).toBeDefined();
    expect(screen.getByText(/test boom/)).toBeDefined();
  });

  it("renders the reset button", () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow={true} />
      </ErrorBoundary>,
    );
    const resetBtn = screen.getByText("Reintentar");
    expect(resetBtn).toBeDefined();
    fireEvent.click(resetBtn);
    // After reset, state is cleared (but the bomb is still throwing, so
    // the error will be caught again — we just verify the click doesn't crash).
  });
});
