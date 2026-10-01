import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { ConnectionStatus, connectionKind } from "./ConnectionStatus";

const base = { ws: "open", health: { service: "neurox" }, isZombie: false };

describe("ConnectionStatus", () => {
  it("renders the status", () => {
    render(<ConnectionStatus conn={base as never} />);
    expect(screen.getByTestId("connection-status")).toBeInTheDocument();
  });

  it("is live when the socket is open and health arrived", () => {
    render(<ConnectionStatus conn={base as never} />);
    const el = screen.getByTestId("connection-status");
    expect(el).toHaveAttribute("data-kind", "live");
    expect(el).toHaveTextContent("en línea");
  });

  it("is offline when the socket is closed", () => {
    render(<ConnectionStatus conn={{ ...base, ws: "closed" } as never} />);
    expect(screen.getByTestId("connection-status")).toHaveAttribute(
      "data-kind",
      "offline",
    );
  });

  it("is connecting before health arrives", () => {
    render(
      <ConnectionStatus conn={{ ...base, health: null } as never} />,
    );
    expect(screen.getByTestId("connection-status")).toHaveAttribute(
      "data-kind",
      "connecting",
    );
  });

  // El WS abierto no garantiza servicio: un heartbeat que no llega
  // significa conexión muerta de verdad.
  it("does not report live when the connection is a zombie", () => {
    render(<ConnectionStatus conn={{ ...base, isZombie: true } as never} />);
    expect(screen.getByTestId("connection-status")).toHaveAttribute(
      "data-kind",
      "connecting",
    );
  });

  it("announces changes politely for screen readers", () => {
    render(<ConnectionStatus conn={base as never} />);
    const el = screen.getByTestId("connection-status");
    expect(el).toHaveAttribute("role", "status");
    expect(el).toHaveAttribute("aria-live", "polite");
  });

  it("hides the decorative glyph from assistive tech", () => {
    const { container } = render(<ConnectionStatus conn={base as never} />);
    expect(container.querySelector(".conn-status__glyph")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });

  describe("connectionKind", () => {
    it("maps the connection state to the three kinds", () => {
      expect(connectionKind(base as never)).toBe("live");
      expect(connectionKind({ ...base, ws: "closed" } as never)).toBe("offline");
      expect(connectionKind({ ...base, health: null } as never)).toBe("connecting");
      expect(connectionKind({ ...base, isZombie: true } as never)).toBe("connecting");
    });
  });
});