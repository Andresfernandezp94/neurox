// Tests para useFullscreen (EP-0025).
// Cubrimos: isSupported, isFullscreen reactivo, isMobile, persistencia,
// toggle, y comportamiento cuando la API no existe.

import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock matchMedia (jsdom no lo trae).
function mockMatchMedia(initialMatches: boolean) {
  const listeners: Array<(e: MediaQueryListEvent) => void> = [];
  const mql = {
    matches: initialMatches,
    media: "(max-width: 1024px)",
    onchange: null,
    addEventListener: (_: string, l: (e: MediaQueryListEvent) => void) => {
      listeners.push(l);
    },
    removeEventListener: (_: string, l: (e: MediaQueryListEvent) => void) => {
      const i = listeners.indexOf(l);
      if (i >= 0) listeners.splice(i, 1);
    },
    dispatchEvent: (e: MediaQueryListEvent) => {
      listeners.forEach((l) => l(e));
    },
  } as unknown as MediaQueryList;
  vi.spyOn(window, "matchMedia").mockReturnValue(mql);
}

describe("useFullscreen", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("toggle llama a requestFullscreen cuando no estamos en fullscreen", async () => {
    mockMatchMedia(true); // mobile
    const requestFn = vi.fn().mockResolvedValue(undefined);
    const exitFn = vi.fn().mockResolvedValue(undefined);
    document.documentElement.requestFullscreen = requestFn;
    document.exitFullscreen = exitFn;

    const { useFullscreen } = await import("./useFullscreen");
    const { result } = renderHook(() => useFullscreen());

    expect(result.current.isSupported).toBe(true);
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isFullscreen).toBe(false);

    await act(async () => {
      await result.current.toggle();
    });

    expect(requestFn).toHaveBeenCalled();
    expect(exitFn).not.toHaveBeenCalled();
    expect(window.localStorage.getItem("neurox:fs-pref")).toBe("on");
  });

  it("persiste la preferencia 'on' al llamar enter()", async () => {
    mockMatchMedia(true);
    const requestFn = vi.fn().mockResolvedValue(undefined);
    document.documentElement.requestFullscreen = requestFn;

    const { useFullscreen } = await import("./useFullscreen");
    const { result } = renderHook(() => useFullscreen());

    await act(async () => {
      await result.current.enter();
    });

    expect(requestFn).toHaveBeenCalled();
    expect(window.localStorage.getItem("neurox:fs-pref")).toBe("on");
    expect(result.current.pref).toBe("on");
  });

  it("persiste la preferencia 'off' al llamar exit()", async () => {
    mockMatchMedia(true);
    window.localStorage.setItem("neurox:fs-pref", "on");
    // Simular que ya estamos en fullscreen.
    Object.defineProperty(document, "fullscreenElement", {
      configurable: true,
      get: () => document.documentElement,
    });
    const exitFn = vi.fn().mockResolvedValue(undefined);
    document.exitFullscreen = exitFn;

    const { useFullscreen } = await import("./useFullscreen");
    const { result } = renderHook(() => useFullscreen());

    expect(result.current.isFullscreen).toBe(true);

    await act(async () => {
      await result.current.exit();
    });

    expect(exitFn).toHaveBeenCalled();
    expect(window.localStorage.getItem("neurox:fs-pref")).toBe("off");
  });

  it("available=false en desktop aunque la API esté disponible", async () => {
    mockMatchMedia(false); // ancho desktop
    Object.defineProperty(window.navigator, "maxTouchPoints", { value: 0, configurable: true });
    document.documentElement.requestFullscreen = vi.fn().mockResolvedValue(undefined);

    const { useFullscreen } = await import("./useFullscreen");
    const { result } = renderHook(() => useFullscreen());

    expect(result.current.isSupported).toBe(true);
    expect(result.current.isMobile).toBe(false);
    expect(result.current.available).toBe(false);
  });

  it("no rompe si localStorage falla (modo privado)", async () => {
    mockMatchMedia(true);
    const requestFn = vi.fn().mockResolvedValue(undefined);
    document.documentElement.requestFullscreen = requestFn;
    const getItemSpy = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new Error("SecurityError");
      });
    const setItemSpy = vi
      .spyOn(Storage.prototype, "setItem")
      .mockImplementation(() => {
        throw new Error("SecurityError");
      });

    const { useFullscreen } = await import("./useFullscreen");
    const { result } = renderHook(() => useFullscreen());

    await act(async () => {
      await result.current.enter();
    });

    expect(requestFn).toHaveBeenCalled();
    getItemSpy.mockRestore();
    setItemSpy.mockRestore();
  });

  it("lee la preferencia persistida al montar", async () => {
    mockMatchMedia(true);
    window.localStorage.setItem("neurox:fs-pref", "on");
    document.documentElement.requestFullscreen = vi.fn().mockResolvedValue(undefined);

    const { useFullscreen } = await import("./useFullscreen");
    const { result } = renderHook(() => useFullscreen());

    expect(result.current.pref).toBe("on");
  });
});