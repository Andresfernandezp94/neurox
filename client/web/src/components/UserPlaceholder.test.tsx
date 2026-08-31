// Tests para UserPlaceholder (EP-0025 — fullscreen toggle en el dropdown).
//
// Cobertura:
// - El item "Pantalla completa" no se renderiza en desktop
// - Sí se renderiza en mobile (cuando la API es compatible)
// - Click en el item llama a toggle() del hook
// - El label/icon cambia cuando isFullscreen es true
// - Theme toggle sigue funcionando (no rompemos nada)

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";

// Mocks de hooks contextuales.
const authMock = vi.fn();
vi.mock("../hooks/useAuth", () => ({
  useAuth: () => authMock(),
}));

const themeMock = vi.fn();
vi.mock("../shared/hooks/useTheme", () => ({
  useTheme: () => themeMock(),
}));

const fsMock = vi.fn();
vi.mock("../shared/hooks/useFullscreen", () => ({
  useFullscreen: () => fsMock(),
}));

vi.mock("../api/auth", () => ({
  logout: vi.fn().mockResolvedValue(undefined),
}));

import { UserPlaceholder } from "./UserPlaceholder";

describe("UserPlaceholder — fullscreen item (EP-0025)", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    authMock.mockReturnValue({
      user: { username: "andres", role: "admin" },
      clear: vi.fn(),
    });
    themeMock.mockReturnValue({ mode: "dark", setMode: vi.fn() });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("no muestra el item de pantalla completa en desktop (available=false)", () => {
    fsMock.mockReturnValue({
      isSupported: true,
      isFullscreen: false,
      isMobile: false,
      pref: "off",
      available: false,
      enter: vi.fn(),
      exit: vi.fn(),
      toggle: vi.fn().mockResolvedValue(undefined),
    });

    render(<UserPlaceholder />);
    fireEvent.click(screen.getByTestId("user-avatar"));

    expect(screen.queryByTestId("user-menu-fullscreen")).toBeNull();
    // Theme sigue presente.
    expect(screen.getByTestId("user-menu-theme")).toBeTruthy();
  });

  it("muestra el item en mobile cuando la API está disponible", () => {
    fsMock.mockReturnValue({
      isSupported: true,
      isFullscreen: false,
      isMobile: true,
      pref: "off",
      available: true,
      enter: vi.fn(),
      exit: vi.fn(),
      toggle: vi.fn().mockResolvedValue(undefined),
    });

    render(<UserPlaceholder />);
    fireEvent.click(screen.getByTestId("user-avatar"));

    const item = screen.getByTestId("user-menu-fullscreen");
    expect(item).toBeTruthy();
    expect(item.textContent).toContain("Pantalla completa");
  });

  it("cambia el label a 'Salir de pantalla completa' cuando isFullscreen=true", () => {
    fsMock.mockReturnValue({
      isSupported: true,
      isFullscreen: true,
      isMobile: true,
      pref: "on",
      available: true,
      enter: vi.fn(),
      exit: vi.fn(),
      toggle: vi.fn().mockResolvedValue(undefined),
    });

    render(<UserPlaceholder />);
    fireEvent.click(screen.getByTestId("user-avatar"));

    const item = screen.getByTestId("user-menu-fullscreen");
    expect(item.textContent).toContain("Salir de pantalla completa");
  });

  it("click en el item llama a toggle() y cierra el dropdown", async () => {
    const toggleSpy = vi.fn().mockResolvedValue(undefined);
    fsMock.mockReturnValue({
      isSupported: true,
      isFullscreen: false,
      isMobile: true,
      pref: "off",
      available: true,
      enter: vi.fn(),
      exit: vi.fn(),
      toggle: toggleSpy,
    });

    render(<UserPlaceholder />);
    fireEvent.click(screen.getByTestId("user-avatar"));
    expect(screen.getByTestId("user-menu")).toBeTruthy();

    await act(async () => {
      fireEvent.click(screen.getByTestId("user-menu-fullscreen"));
    });

    expect(toggleSpy).toHaveBeenCalledTimes(1);
    // Después del click el dropdown se cierra.
    await vi.waitFor(() => {
      expect(screen.queryByTestId("user-menu")).toBeNull();
    });
  });

  it("Theme toggle sigue funcionando (no rompemos el feature existente)", () => {
    const setModeSpy = vi.fn();
    themeMock.mockReturnValue({ mode: "light", setMode: setModeSpy });
    fsMock.mockReturnValue({
      isSupported: false,
      isFullscreen: false,
      isMobile: false,
      pref: "off",
      available: false,
      enter: vi.fn(),
      exit: vi.fn(),
      toggle: vi.fn(),
    });

    render(<UserPlaceholder />);
    fireEvent.click(screen.getByTestId("user-avatar"));
    fireEvent.click(screen.getByTestId("user-menu-theme"));

    expect(setModeSpy).toHaveBeenCalledWith("dark");
  });
});