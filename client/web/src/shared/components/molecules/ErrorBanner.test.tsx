import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ErrorBanner } from "./ErrorBanner";

describe("ErrorBanner", () => {
  it("renders children", () => {
    render(<ErrorBanner>Something failed</ErrorBanner>);
    expect(screen.getByText("Something failed")).toBeInTheDocument();
  });

  it("defaults to error variant with role=alert", () => {
    render(<ErrorBanner>boom</ErrorBanner>);
    const banner = screen.getByRole("alert");
    expect(banner).toHaveClass("error-banner");
  });

  it("warn variant uses role=status", () => {
    render(<ErrorBanner variant="warn">careful</ErrorBanner>);
    const banner = screen.getByRole("status");
    expect(banner).toHaveClass("error-banner--warn");
  });

  it("info variant uses role=status", () => {
    render(<ErrorBanner variant="info">fyi</ErrorBanner>);
    const banner = screen.getByRole("status");
    expect(banner).toHaveClass("error-banner--info");
  });

  it("does not render dismiss button by default", () => {
    render(<ErrorBanner>x</ErrorBanner>);
    expect(
      screen.queryByRole("button", { name: "Dismiss" }),
    ).not.toBeInTheDocument();
  });

  it("renders dismiss button when onDismiss provided", () => {
    render(<ErrorBanner onDismiss={() => {}}>x</ErrorBanner>);
    expect(
      screen.getByRole("button", { name: "Dismiss" }),
    ).toBeInTheDocument();
  });

  it("calls onDismiss when clicked", () => {
    const handle = vi.fn();
    render(<ErrorBanner onDismiss={handle}>x</ErrorBanner>);
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(handle).toHaveBeenCalledTimes(1);
  });

  it("merges custom className", () => {
    render(<ErrorBanner className="extra">x</ErrorBanner>);
    expect(screen.getByRole("alert")).toHaveClass("extra");
  });
});
