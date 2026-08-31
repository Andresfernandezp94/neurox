// useHideHeader — runtime toggle to hide the AppHeader without deleting it
// (per user request: "ocultar el header, no eliminarlo" + "en mobile no
// se debe ver el header").
//
// Precedence (highest first):
//   1. URL query param `?hideHeader=1` (hide) / `?hideHeader=0` (show).
//      When present, mirrored to localStorage so the choice persists
//      across reloads (otherwise the user would have to keep adding
//      the param to every URL).
//   2. localStorage 'neurox:hide-header' — `"1"` → hidden, `"0"` →
//      shown. An empty/missing key means "use the viewport default".
//   3. Viewport default — mobile (max-width: 1024px) → hidden,
//      desktop → visible. Matches the breakpoint used by Sidebar and
//      tokens.css mobile rules (Sidebar pinned=false on mobile,
//      .app-main margin-left collapse).
//
// Reactivity: subscribes to `matchMedia.change` so a window resize
// from desktop → mobile flips the default to "hidden" (and vice versa)
// unless the user explicitly chose otherwise via URL/localStorage.
//
// SSR-safe: when `window` is undefined, the hook returns false
// (desktop default).

import { useEffect, useState } from "react";

const STORAGE_KEY = "neurox:hide-header";
const MOBILE_QUERY = "(max-width: 1024px)";

function isTruthyString(s: string): boolean {
  return s === "1" || s.toLowerCase() === "true";
}

function isFalsyString(s: string): boolean {
  return s === "0" || s.toLowerCase() === "false";
}

type ExplicitChoice = "hidden" | "shown";

/**
 * Pure read of URL + localStorage. Returns `null` when there's no
 * explicit override (so the viewport default should apply).
 */
function readExplicit(): ExplicitChoice | null {
  if (typeof window === "undefined") return null;

  // URL takes precedence over localStorage.
  const params = new URLSearchParams(window.location.search);
  const urlValue = params.get("hideHeader");
  if (urlValue !== null) {
    if (isTruthyString(urlValue)) return "hidden";
    if (isFalsyString(urlValue)) return "shown";
    return "hidden"; // unknown value → treat as truthy (hide)
  }

  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "1") return "hidden";
    if (stored === "0") return "shown";
  } catch {
    // ignore (quota / private mode)
  }

  return null;
}

/**
 * Side-effect helper: when the URL has `hideHeader=…`, mirror the
 * resolved value to localStorage so the choice survives reloads.
 */
function mirrorUrlToStorage(): void {
  if (typeof window === "undefined") return;
  const params = new URLSearchParams(window.location.search);
  const urlValue = params.get("hideHeader");
  if (urlValue === null) return;
  let choice: ExplicitChoice;
  if (isTruthyString(urlValue)) choice = "hidden";
  else if (isFalsyString(urlValue)) choice = "shown";
  else choice = "hidden";
  try {
    window.localStorage.setItem(STORAGE_KEY, choice === "hidden" ? "1" : "0");
  } catch {
    // ignore
  }
}

function isMobileViewport(): boolean {
  if (typeof window === "undefined") return false;
  if (typeof window.matchMedia !== "function") return false;
  try {
    return window.matchMedia(MOBILE_QUERY).matches;
  } catch {
    return false;
  }
}

function computeHidden(): boolean {
  const explicit = readExplicit();
  if (explicit === "hidden") return true;
  if (explicit === "shown") return false;
  return isMobileViewport(); // default: mobile → hidden
}

/**
 * Returns `true` if the AppHeader should be hidden.
 *
 * - URL `?hideHeader=1` or `=0` overrides (and persists to localStorage).
 * - localStorage `neurox:hide-header` ("1" / "0") overrides.
 * - Otherwise the viewport decides: ≤1024px wide → hidden,
 *   wider → visible. Listens to viewport resize so the default
 *   tracks the breakpoint live.
 */
export function useHideHeader(): boolean {
  // Mirror URL → localStorage once on mount (so reloads keep the choice).
  useEffect(() => {
    mirrorUrlToStorage();
  }, []);

  const [hidden, setHidden] = useState<boolean>(computeHidden);

  // React to viewport resize: if no explicit override, recompute based
  // on the breakpoint. If the user has an explicit choice, respect it.
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (typeof window.matchMedia !== "function") return;
    const mql = window.matchMedia(MOBILE_QUERY);
    const onChange = () => {
      if (readExplicit() === null) {
        setHidden(mql.matches);
      }
    };
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  return hidden;
}