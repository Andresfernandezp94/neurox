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
});
