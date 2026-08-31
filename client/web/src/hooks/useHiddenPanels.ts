// useHiddenPanels — dev/refactor helper to hide specific UI panels.
// Lets you iterate on a refactor without the noise of unrelated
// panels getting in the way.
//
// Two ways a panel can be hidden:
//
// 1. DEFAULT-hidden (always hidden unless explicitly shown):
//    Listed in `DEFAULT_HIDDEN` below. Toggle by passing its id to
//    `?show=` (e.g. `?show=chat-tabs`).
//
// 2. URL-hidden (in addition to the defaults):
//    `?hide=foo,bar` adds these ids on top of the defaults. To
//    *re-show* a default-hidden panel, use `?show=`.
//
// Usage:
//   const { isHidden } = useHiddenPanels();
//   if (!isHidden("contextBar")) return <ContextBar />;
//   // or
//   <PanelToggle id="contextBar"><ContextBar /></PanelToggle>
//
// URL syntax:
//   (no params)                       → defaults hidden
//   ?hide=contextBar,workspaceBar     → defaults + URL-listed hidden
//   ?show=chat-tabs                   → reveal a default-hidden panel
//   ?hide=foo&show=bar                → combined (processed in order)
//
// Hidden panels are unmounted, not just CSS-hidden — so effects,
// refs, and event handlers in the panel code don't run. Useful when
// debugging a refactor: if a bug appears after toggling a panel
// back on, it's in that panel's code (or what calls it).
//
// SSR-safe: returns `DEFAULT_HIDDEN` when `window` is undefined.

import { useCallback, useMemo } from "react";

/** Panel ids that are hidden by default. Override per-panel with `?show=`. */
export const DEFAULT_HIDDEN: readonly string[] = [
  // 2026-08-15: chat-tabs se removió de los defaults — el usuario
  // lo quiere visible. chat-search-bar y chat-mic siguen ocultos
  // (son features secundarias que no son parte del chat principal).
  // Re-enable locally with `?show=<id>` while testing.
  "chat-search-bar",
  "chat-mic",
];

export interface UseHiddenPanelsResult {
  /** True if the panel is currently hidden (default or URL-added). */
  isHidden: (id: string) => boolean;
  /** The set of currently hidden panel ids (read-only, for debugging). */
  hidden: ReadonlySet<string>;
}

function parseList(raw: string | null): Set<string> {
  if (raw === null) return new Set();
  return new Set(
    raw
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
  );
}

export function useHiddenPanels(): UseHiddenPanelsResult {
  const hidden = useMemo<ReadonlySet<string>>(() => {
    if (typeof window === "undefined") {
      return new Set(DEFAULT_HIDDEN);
    }
    const params = new URLSearchParams(window.location.search);
    const urlHidden = parseList(params.get("hide"));
    const shown = parseList(params.get("show"));
    // Union of (defaults + ?hide=) minus ?show=. The URL can only
    // ADD to the default set; `?show=` removes panels from the set.
    const merged = new Set(DEFAULT_HIDDEN);
    for (const id of urlHidden) merged.add(id);
    for (const id of shown) merged.delete(id);
    return merged;
  }, []);

  const isHidden = useCallback(
    (id: string) => hidden.has(id),
    [hidden],
  );

  return useMemo(
    () => ({ hidden, isHidden }),
    [hidden, isHidden],
  );
}