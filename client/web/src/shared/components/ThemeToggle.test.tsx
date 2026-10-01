import { render, screen } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { ThemeToggle } from "./ThemeToggle";

describe("ThemeToggle", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("renders the toggle button", () => {
    render(<ThemeToggle />);
    expect(screen.getByTestId("theme-toggle")).toBeInTheDocument();
  });

  it("has an accessible label", () => {
    render(<ThemeToggle />);
    expect(screen.getByRole("button", { name: /cambiar tema/i })).toBeInTheDocument();
  });

  it("accepts a custom className for positioning hooks", () => {
    render(<ThemeToggle className="login-screen__theme-toggle" />);
    expect(screen.getByTestId("theme-toggle").className).toContain(
      "login-screen__theme-toggle",
    );
  });

  it("renders an svg icon that follows the theme", () => {
    // Sin modo persistido, useTheme arranca en "dark" (dark-first), así
    // que debe verse el ícono de sol (invita a pasar a claro).
    const { container } = render(<ThemeToggle />);
    const svg = container.querySelector("svg");
    expect(svg).toBeInTheDocument();
    expect(svg).toHaveAttribute("aria-hidden", "true");
  });
});