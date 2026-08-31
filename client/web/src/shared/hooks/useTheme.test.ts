// Tests para useTheme hook.
// Port one-way desde agent-studio/src-ui/modules/shared/hooks/useTheme.ts

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  useTheme,
  getSystemTheme,
  resolveTheme,
  applyThemeToDom,
  type ThemeMode,
} from "./useTheme";

describe("useTheme", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("getSystemTheme", () => {
    it("returns 'light' when prefers-color-scheme is light", () => {
      vi.spyOn(window, "matchMedia").mockImplementation(
        () =>
          ({
            matches: true,
            media: "(prefers-color-scheme: light)",
            addEventListener: vi.fn(),
            removeEventListener: vi.fn(),
            addListener: vi.fn(),
            removeListener: vi.fn(),
            dispatchEvent: vi.fn(),
            onchange: null,
          }) as unknown as MediaQueryList,
      );
      expect(getSystemTheme()).toBe("light");
    });

    it("returns 'dark' when prefers-color-scheme is dark", () => {
      vi.spyOn(window, "matchMedia").mockImplementation(
        () =>
          ({
            matches: false,
            media: "(prefers-color-scheme: light)",
            addEventListener: vi.fn(),
            removeEventListener: vi.fn(),
            addListener: vi.fn(),
            removeListener: vi.fn(),
            dispatchEvent: vi.fn(),
            onchange: null,
          }) as unknown as MediaQueryList,
      );
      expect(getSystemTheme()).toBe("dark");
    });
  });

  describe("resolveTheme", () => {
    it("resolves 'system' to current OS theme", () => {
      vi.spyOn(window, "matchMedia").mockImplementation(
        () =>
          ({
            matches: true,
            media: "(prefers-color-scheme: light)",
            addEventListener: vi.fn(),
            removeEventListener: vi.fn(),
            addListener: vi.fn(),
            removeListener: vi.fn(),
            dispatchEvent: vi.fn(),
            onchange: null,
          }) as unknown as MediaQueryList,
      );
      expect(resolveTheme("system")).toBe("light");
    });

    it("returns 'light' as-is", () => {
      expect(resolveTheme("light")).toBe("light");
    });

    it("returns 'dark' as-is", () => {
      expect(resolveTheme("dark")).toBe("dark");
    });
  });

  describe("applyThemeToDom", () => {
    it("sets data-theme attribute on <html>", () => {
      applyThemeToDom("light");
      expect(document.documentElement.getAttribute("data-theme")).toBe("light");

      applyThemeToDom("dark");
      expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    });
  });

  describe("useTheme hook", () => {
    it("starts with default 'dark' when no stored preference", () => {
      const { result } = renderHook(() => useTheme());
      expect(result.current.mode).toBe("dark");
    });

    it("restores mode from localStorage", () => {
      localStorage.setItem("theme-mode", "light");
      const { result } = renderHook(() => useTheme());
      expect(result.current.mode).toBe("light");
    });

    it("setMode updates the mode and applies to DOM", () => {
      const { result } = renderHook(() => useTheme());

      act(() => {
        result.current.setMode("light");
      });

      expect(result.current.mode).toBe("light");
      expect(document.documentElement.getAttribute("data-theme")).toBe("light");
      expect(localStorage.getItem("theme-mode")).toBe("light");
    });

    it("setMode persists across renders", () => {
      const { result, rerender } = renderHook(() => useTheme());

      act(() => {
        result.current.setMode("system");
      });

      rerender();

      expect(result.current.mode).toBe("system");
      expect(localStorage.getItem("theme-mode")).toBe("system");
    });

    it("cycles through system → light → dark → system", () => {
      const { result } = renderHook(() => useTheme());
      const cycle: ThemeMode[] = ["system", "light", "dark"];

      for (const mode of cycle) {
        act(() => {
          result.current.setMode(mode);
        });
        expect(result.current.mode).toBe(mode);
      }
    });
  });
});
