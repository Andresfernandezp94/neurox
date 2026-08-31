// useHideHeader — tests for the runtime toggle that hides the AppHeader
// (per user request: "ocultar el header, no eliminarlo" + "en mobile
// no se debe ver el header").
//
// Precedence under test:
//   1. URL ?hideHeader=1 / ?hideHeader=0 (highest) — also mirrors to
//      localStorage so the choice survives reloads.
//   2. localStorage 'neurox:hide-header' ("1" → hidden, "0" → shown).
//   3. Viewport default → mobile (≤1024px) hidden, desktop visible.
//      Live resize is honored unless an explicit override is set.

import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useHideHeader } from "./useHideHeader";

const STORAGE_KEY = "neurox:hide-header";
const MOBILE_QUERY = "(max-width: 1024px)";

function setUrl(search: string) {
  const url = new URL(window.location.href);
  url.search = search;
  window.history.replaceState({}, "", url.toString());
}

interface MockMQL {
  matches: boolean;
  media: string;
  onchange: null;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
  addListener: ReturnType<typeof vi.fn>;
  removeListener: ReturnType<typeof vi.fn>;
  dispatchEvent: ReturnType<typeof vi.fn>;
  /** Trigger the registered "change" listeners (simulates viewport resize). */
  fireChange: (matches: boolean) => void;
}

/**
 * Install a controllable matchMedia mock. Returns the underlying
 * MediaQueryList mock so tests can fire `change` events.
 */
function mockMatchMedia(isMobile: boolean): MockMQL {
  const listeners = new Set<(e: MediaQueryListEvent) => void>();
  const mql: MockMQL = {
    matches: isMobile,
    media: MOBILE_QUERY,
    onchange: null,
    addEventListener: vi.fn((_type: string, cb: (e: MediaQueryListEvent) => void) => {
      listeners.add(cb);
    }),
    removeEventListener: vi.fn((_type: string, cb: (e: MediaQueryListEvent) => void) => {
      listeners.delete(cb);
    }),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
    fireChange(matches: boolean) {
      mql.matches = matches;
      for (const cb of listeners) cb({ matches, media: MOBILE_QUERY } as MediaQueryListEvent);
    },
  };
  vi.spyOn(window, "matchMedia").mockImplementation(
    () => mql as unknown as MediaQueryList,
  );
  return mql;
}

describe("useHideHeader", () => {
  beforeEach(() => {
    window.localStorage.clear();
    setUrl("");
  });

  afterEach(() => {
    window.localStorage.clear();
    setUrl("");
    vi.restoreAllMocks();
  });

  describe("default (no overrides)", () => {
    it("returns true (hidden) by default on a mobile viewport", () => {
      mockMatchMedia(true);
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(true);
    });

    it("returns false (visible) by default on a desktop viewport", () => {
      mockMatchMedia(false);
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);
    });
  });

  describe("localStorage overrides", () => {
    it("reads hidden from localStorage when URL has no param", () => {
      mockMatchMedia(false); // desktop default would be visible
      window.localStorage.setItem(STORAGE_KEY, "1");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(true);
    });

    it("reads shown from localStorage when URL has no param", () => {
      mockMatchMedia(true); // mobile default would be hidden
      window.localStorage.setItem(STORAGE_KEY, "0");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);
    });

    it("ignores unknown localStorage values", () => {
      mockMatchMedia(false);
      window.localStorage.setItem(STORAGE_KEY, "yes-please");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);
    });
  });

  describe("URL overrides (highest priority)", () => {
    it("?hideHeader=1 wins over localStorage=0 and over desktop default", () => {
      mockMatchMedia(false);
      window.localStorage.setItem(STORAGE_KEY, "0");
      setUrl("?hideHeader=1");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(true);
    });

    it("?hideHeader=0 wins over localStorage=1 and over mobile default", () => {
      mockMatchMedia(true);
      window.localStorage.setItem(STORAGE_KEY, "1");
      setUrl("?hideHeader=0");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);
    });

    it("?hideHeader=0 mirrors to localStorage as '0' (explicit show)", () => {
      mockMatchMedia(true);
      setUrl("?hideHeader=0");
      renderHook(() => useHideHeader());
      // El side-effect corre en useEffect, así que envolvemos en act()
      // y esperamos a que termine el flush de microtasks.
      return Promise.resolve().then(() => {
        expect(window.localStorage.getItem(STORAGE_KEY)).toBe("0");
      });
    });

    it("?hideHeader=1 mirrors to localStorage as '1' so it survives reload", () => {
      mockMatchMedia(false);
      setUrl("?hideHeader=1");
      renderHook(() => useHideHeader());
      return Promise.resolve().then(() => {
        expect(window.localStorage.getItem(STORAGE_KEY)).toBe("1");
      });
    });

    it("?hideHeader=true is treated as truthy", () => {
      mockMatchMedia(false);
      setUrl("?hideHeader=true");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(true);
    });

    it("?hideHeader=false is treated as falsy", () => {
      mockMatchMedia(true);
      setUrl("?hideHeader=false");
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);
    });
  });

  describe("viewport resize reactivity", () => {
    it("flips to hidden when viewport shrinks from desktop to mobile (no override)", () => {
      const mql = mockMatchMedia(false); // start desktop
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);

      act(() => {
        mql.fireChange(true); // simulate resize to mobile
      });
      expect(result.current).toBe(true);
    });

    it("flips to visible when viewport grows from mobile to desktop (no override)", () => {
      const mql = mockMatchMedia(true); // start mobile
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(true);

      act(() => {
        mql.fireChange(false); // simulate resize to desktop
      });
      expect(result.current).toBe(false);
    });

    it("explicit 'shown' localStorage survives a viewport resize to mobile", () => {
      const mql = mockMatchMedia(false);
      window.localStorage.setItem(STORAGE_KEY, "0"); // explicit show
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(false);

      act(() => {
        mql.fireChange(true); // resize to mobile
      });
      // Explicit override wins → still shown.
      expect(result.current).toBe(false);
    });

    it("explicit 'hidden' localStorage survives a viewport resize to desktop", () => {
      const mql = mockMatchMedia(true);
      window.localStorage.setItem(STORAGE_KEY, "1"); // explicit hide
      const { result } = renderHook(() => useHideHeader());
      expect(result.current).toBe(true);

      act(() => {
        mql.fireChange(false); // resize to desktop
      });
      // Explicit override wins → still hidden.
      expect(result.current).toBe(true);
    });
  });
});