// useHiddenPanels — dev/refactor helper for hiding UI panels.
// Two ways to hide: DEFAULT_HIDDEN list (always) and ?hide= URL param
// (additive). `?show=` reveals a panel even if it's in the default list.

import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useHiddenPanels } from "./useHiddenPanels";

function setUrl(search: string) {
  const url = new URL(window.location.href);
  url.search = search;
  window.history.replaceState({}, "", url.toString());
}

describe("useHiddenPanels", () => {
  beforeEach(() => {
    setUrl("");
  });

  afterEach(() => {
    setUrl("");
  });

  describe("defaults", () => {
    it("hides DEFAULT_HIDDEN panels (chat-search-bar, chat-mic) with no URL params", () => {
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.isHidden("chat-tabs")).toBe(false);
      expect(result.current.isHidden("chat-search-bar")).toBe(true);
      expect(result.current.isHidden("chat-mic")).toBe(true);
    });

    it("does NOT hide other panels by default", () => {
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.isHidden("chat-context-bar")).toBe(false);
      expect(result.current.isHidden("chat-workspace-bar")).toBe(false);
      expect(result.current.isHidden("chat-input")).toBe(false);
    });
  });

  describe("?hide= (additive)", () => {
    it("hides a single panel in addition to defaults", () => {
      setUrl("?hide=contextBar");
      const { result } = renderHook(() => useHiddenPanels());
      // chat-tabs no está en defaults — solo los que sí
      expect(result.current.isHidden("chat-tabs")).toBe(false);
      // The added one
      expect(result.current.isHidden("contextBar")).toBe(true);
      // Unrelated panel
      expect(result.current.isHidden("chat-input")).toBe(false);
    });

    it("reads multiple panels (comma-separated)", () => {
      setUrl("?hide=contextBar,workspaceBar,history");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.hidden.size).toBe(5); // 2 defaults (chat-search-bar, chat-mic) + 3 added
      expect(result.current.isHidden("contextBar")).toBe(true);
      expect(result.current.isHidden("workspaceBar")).toBe(true);
      expect(result.current.isHidden("history")).toBe(true);
    });

    it("handles URL-encoded comma (%2C)", () => {
      setUrl("?hide=contextBar%2CworkspaceBar");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.hidden.size).toBe(4); // 2 defaults + 2 added
      expect(result.current.isHidden("contextBar")).toBe(true);
      expect(result.current.isHidden("workspaceBar")).toBe(true);
    });

    it("trims whitespace around panel ids", () => {
      setUrl("?hide=contextBar%20,%20workspaceBar");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.isHidden("contextBar")).toBe(true);
      expect(result.current.isHidden("workspaceBar")).toBe(true);
    });

    it("ignores empty entries", () => {
      setUrl("?hide=contextBar,,,workspaceBar,");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.hidden.has("contextBar")).toBe(true);
      expect(result.current.hidden.has("workspaceBar")).toBe(true);
    });

    it("treats empty ?hide= as no hidden panels (BUT defaults still apply)", () => {
      setUrl("?hide=");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.hidden.size).toBe(2); // defaults only (chat-search-bar, chat-mic)
      expect(result.current.isHidden("chat-tabs")).toBe(false);
    });
  });

  describe("?show= (override defaults)", () => {
    it("reveals a default-hidden panel", () => {
      setUrl("?show=chat-tabs");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.isHidden("chat-tabs")).toBe(false);
      // Other default still applies
      expect(result.current.isHidden("chat-search-bar")).toBe(true);
    });

    it("reveals multiple default-hidden panels", () => {
      setUrl("?show=chat-tabs,chat-search-bar,chat-mic");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.hidden.size).toBe(0);
      expect(result.current.isHidden("chat-tabs")).toBe(false);
      expect(result.current.isHidden("chat-search-bar")).toBe(false);
      expect(result.current.isHidden("chat-mic")).toBe(false);
    });

    it("combined ?hide=X&show=Y works (defaults + hide + show)", () => {
      setUrl("?hide=contextBar&show=chat-tabs");
      const { result } = renderHook(() => useHiddenPanels());
      expect(result.current.isHidden("contextBar")).toBe(true);
      expect(result.current.isHidden("chat-tabs")).toBe(false);
      expect(result.current.isHidden("chat-search-bar")).toBe(true);
    });
  });

  describe("stability", () => {
    it("returns the same isHidden reference across renders (stable)", () => {
      setUrl("?hide=contextBar");
      const { result, rerender } = renderHook(() => useHiddenPanels());
      const first = result.current.isHidden;
      rerender();
      const second = result.current.isHidden;
      expect(first).toBe(second);
    });
  });
});