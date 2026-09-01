import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { StatusBar } from "./StatusBar";

describe("StatusBar", () => {
  it("renders version", () => {
    render(<StatusBar version="0.4.0" status="ok" />);
    expect(screen.getByText("v0.4.0")).toBeInTheDocument();
  });

  it("renders 'ok' badge for status ok", () => {
    render(<StatusBar version="1.0" status="ok" />);
    expect(screen.getByText("ok")).toBeInTheDocument();
  });

  it("renders 'degraded' badge", () => {
    render(<StatusBar version="1.0" status="degraded" />);
    expect(screen.getByText("degraded")).toBeInTheDocument();
  });

  it("renders 'error' badge", () => {
    render(<StatusBar version="1.0" status="error" />);
    expect(screen.getByText("error")).toBeInTheDocument();
  });

  it("renders 'connecting…' badge", () => {
    render(<StatusBar version="1.0" status="connecting" />);
    expect(screen.getByText("connecting…")).toBeInTheDocument();
  });

  it("formats uptime in hours", () => {
    render(<StatusBar version="1.0" status="ok" uptimeSeconds={7260} />);
    expect(screen.getByText("↑ 2h 1m")).toBeInTheDocument();
  });

  it("formats uptime in minutes", () => {
    render(<StatusBar version="1.0" status="ok" uptimeSeconds={125} />);
    expect(screen.getByText("↑ 2m 5s")).toBeInTheDocument();
  });

  it("formats uptime in seconds only", () => {
    render(<StatusBar version="1.0" status="ok" uptimeSeconds={45} />);
    expect(screen.getByText("↑ 45s")).toBeInTheDocument();
  });

  it("does not render uptime when omitted", () => {
    const { container } = render(<StatusBar version="1.0" status="ok" />);
    expect(container.textContent).not.toContain("↑");
  });

  // DOT-fix: el badge debe incluir un .badge__dot cuando showDot=true
  // para que el usuario vea de un vistazo si el daemon está conectado,
  // degradado, o caído. CSS ya existía (atoms.css .badge__dot +
  // .badge--success/.warn/.info .badge__dot { animation: badge-pulse }).
  it("does not render dot by default", () => {
    const { container } = render(<StatusBar version="1.0" status="ok" />);
    expect(container.querySelector(".badge__dot")).not.toBeInTheDocument();
  });

  it("renders dot when showDot=true", () => {
    const { container } = render(
      <StatusBar version="1.0" status="ok" showDot />
    );
    const dot = container.querySelector(".badge__dot");
    expect(dot).toBeInTheDocument();
    expect(dot).toHaveAttribute("aria-hidden", "true");
  });

  it("renders dot for all four statuses when showDot=true", () => {
    for (const status of ["ok", "degraded", "error", "connecting"] as const) {
      const { container } = render(
        <StatusBar version="1.0" status={status} showDot />
      );
      const dot = container.querySelector(".badge__dot");
      const badge = container.querySelector(".badge");
      expect(dot, `dot missing for status=${status}`).toBeInTheDocument();
      expect(badge, `badge missing for status=${status}`).toBeInTheDocument();
    }
  });
});
