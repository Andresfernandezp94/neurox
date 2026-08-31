import { useState, useEffect, useCallback } from "react";

export type ThemeMode = "system" | "light" | "dark";

const MODE_KEY = "theme-mode";
const LEGACY_OVERRIDE_KEY = "theme-override";
const LEGACY_THEME_KEY = "theme";

export function getSystemTheme(): "dark" | "light" {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

/** Resolve the effective theme (resolves "system" to OS preference). */
export function resolveTheme(mode: ThemeMode): "dark" | "light" {
  return mode === "system" ? getSystemTheme() : mode;
}

/** Apply the effective theme to the <html> element so CSS tokens match. */
export function applyThemeToDom(mode: ThemeMode) {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", resolveTheme(mode));
}

function getInitialMode(): ThemeMode {
  if (typeof window === "undefined") return "dark";
  const stored = localStorage.getItem(MODE_KEY);
  if (stored === "system" || stored === "light" || stored === "dark") {
    return stored;
  }
  if (localStorage.getItem(LEGACY_OVERRIDE_KEY) === "1") {
    const legacy = localStorage.getItem(LEGACY_THEME_KEY);
    localStorage.removeItem(LEGACY_OVERRIDE_KEY);
    localStorage.removeItem(LEGACY_THEME_KEY);
    if (legacy === "light" || legacy === "dark") return legacy;
  }
  // Default to dark — this app is dark-first by design.
  return "dark";
}

export function useTheme() {
  const [mode, setModeState] = useState<ThemeMode>(getInitialMode);

  // Persist + apply to DOM on every change.
  useEffect(() => {
    localStorage.setItem(MODE_KEY, mode);
    applyThemeToDom(mode);
  }, [mode]);

  // React to OS preference changes when in "system" mode.
  useEffect(() => {
    const mql = window.matchMedia("(prefers-color-scheme: light)");
    const handler = () => {
      if (mode === "system") applyThemeToDom("system");
    };
    mql.addEventListener?.("change", handler);
    return () => mql.removeEventListener?.("change", handler);
  }, [mode]);

  const setMode = useCallback((m: ThemeMode) => {
    setModeState(m);
    applyThemeToDom(m);
  }, []);

  return { mode, setMode };
}
