// useFullscreen — hook que envuelve la Fullscreen API para ofrecer una
// opción "pantalla completa" en mobile (EP-0025).
//
// Capacidades:
// - isSupported: true si el navegador expone element.requestFullscreen().
// - isFullscreen: estado reactivo (escucha 'fullscreenchange').
// - isMobile: heurística de viewport móvil (matchMedia o maxTouchPoints).
// - toggle(): entra/sale de fullscreen.
// - enter() / exit(): explícitos.
//
// Persistencia: la preferencia del usuario se guarda en localStorage bajo
// `neurox:fs-pref` con valores 'on' | 'off'. En el mount, si la pref es
// 'on' y la API es compatible y estamos en mobile, se intenta entrar
// automáticamente. El efecto es best-effort: si el browser bloquea la
// transición (ej. requiere gesto del usuario), se ignora silenciosamente.

import { useCallback, useEffect, useState } from "react";

const PREF_KEY = "neurox:fs-pref";

export type FsPref = "on" | "off";

function getDocumentFsElement(): Element | null {
  if (typeof document === "undefined") return null;
  // Soporta las variantes prefixed (webkit / moz) para navegadores viejos.
  const d = document as Document & {
    webkitFullscreenElement?: Element | null;
    mozFullScreenElement?: Element | null;
    msFullscreenElement?: Element | null;
  };
  return (
    document.fullscreenElement ??
    d.webkitFullscreenElement ??
    d.mozFullScreenElement ??
    d.msFullscreenElement ??
    null
  );
}

function isFsApiSupported(): boolean {
  if (typeof document === "undefined") return false;
  const d = document as Document & {
    webkitExitFullscreen?: () => Promise<void> | void;
    mozCancelFullScreen?: () => Promise<void> | void;
    msExitFullscreen?: () => Promise<void> | void;
  };
  const el = document.documentElement as HTMLElement & {
    webkitRequestFullscreen?: () => Promise<void> | void;
    mozRequestFullScreen?: () => Promise<void> | void;
    msRequestFullscreen?: () => Promise<void> | void;
  };
  return Boolean(
    el.requestFullscreen ||
      el.webkitRequestFullscreen ||
      el.mozRequestFullScreen ||
      el.msRequestFullscreen ||
      d.webkitExitFullscreen ||
      d.mozCancelFullScreen ||
      d.msExitFullscreen
  );
}

function isMobileViewport(): boolean {
  if (typeof window === "undefined") return false;
  // 1) matchMedia del breakpoint mobile que ya usa la app (Sidebar).
  const narrow =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(max-width: 1024px)").matches;
  // 2) Heurística de touch device.
  const touch =
    typeof navigator !== "undefined" &&
    (navigator.maxTouchPoints > 0 ||
      // @ts-expect-error — legacy Safari
      navigator.msMaxTouchPoints > 0);
  return Boolean(narrow || touch);
}

function readPref(): FsPref {
  if (typeof window === "undefined") return "off";
  try {
    const raw = window.localStorage.getItem(PREF_KEY);
    return raw === "on" ? "on" : "off";
  } catch {
    return "off";
  }
}

function writePref(p: FsPref): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(PREF_KEY, p);
  } catch {
    // ignore (quota, private mode)
  }
}

async function requestFs(): Promise<void> {
  if (typeof document === "undefined") return;
  const el = document.documentElement as HTMLElement & {
    webkitRequestFullscreen?: () => Promise<void> | void;
    mozRequestFullScreen?: () => Promise<void> | void;
    msRequestFullscreen?: () => Promise<void> | void;
  };
  const fn =
    el.requestFullscreen ||
    el.webkitRequestFullscreen ||
    el.mozRequestFullScreen ||
    el.msRequestFullscreen;
  if (fn) {
    await fn.call(el);
  }
}

async function exitFs(): Promise<void> {
  if (typeof document === "undefined") return;
  const d = document as Document & {
    webkitExitFullscreen?: () => Promise<void> | void;
    mozCancelFullScreen?: () => Promise<void> | void;
    msExitFullscreen?: () => Promise<void> | void;
  };
  const fn =
    document.exitFullscreen ||
    d.webkitExitFullscreen ||
    d.mozCancelFullScreen ||
    d.msExitFullscreen;
  if (fn) {
    await fn.call(document);
  }
}

export interface UseFullscreenValue {
  /** true si el navegador expone la Fullscreen API. */
  isSupported: boolean;
  /** true si actualmente hay un elemento en pantalla completa. */
  isFullscreen: boolean;
  /** true si el viewport parece mobile (breakpoint o touch). */
  isMobile: boolean;
  /** Preferencia persistida del usuario. */
  pref: FsPref;
  /** true solo si tiene sentido ofrecer el botón en la UI actual. */
  available: boolean;
  /** Solicita entrar a pantalla completa (resuelve aunque el browser la bloquee). */
  enter: () => Promise<void>;
  /** Solicita salir de pantalla completa. */
  exit: () => Promise<void>;
  /** Alterna entre entrar/salir y actualiza la preferencia persistida. */
  toggle: () => Promise<void>;
}

export function useFullscreen(): UseFullscreenValue {
  const [isSupported] = useState<boolean>(isFsApiSupported);
  const [isFullscreen, setIsFullscreen] = useState<boolean>(() =>
    Boolean(getDocumentFsElement())
  );
  const [isMobile, setIsMobile] = useState<boolean>(() => isMobileViewport());
  const [pref, setPref] = useState<FsPref>(() => readPref());

  // Escuchar cambios de fullscreen (usuario pulsando ESC, etc.).
  useEffect(() => {
    if (!isSupported) return;
    const handler = () => setIsFullscreen(Boolean(getDocumentFsElement()));
    document.addEventListener("fullscreenchange", handler);
    document.addEventListener("webkitfullscreenchange", handler);
    document.addEventListener("mozfullscreenchange", handler);
    document.addEventListener("MSFullscreenChange", handler);
    return () => {
      document.removeEventListener("fullscreenchange", handler);
      document.removeEventListener("webkitfullscreenchange", handler);
      document.removeEventListener("mozfullscreenchange", handler);
      document.removeEventListener("MSFullscreenChange", handler);
    };
  }, [isSupported]);

  // Re-evaluar isMobile si cambia el viewport (rotación, resize).
  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }
    const mql = window.matchMedia("(max-width: 1024px)");
    const handler = () => setIsMobile(isMobileViewport());
    mql.addEventListener?.("change", handler);
    window.addEventListener("resize", handler);
    return () => {
      mql.removeEventListener?.("change", handler);
      window.removeEventListener("resize", handler);
    };
  }, []);

  // Al montar: si la preferencia es 'on' y podemos, intentar entrar.
  // Best-effort: si el browser exige un gesto del usuario, fallará silencioso.
  useEffect(() => {
    if (!isSupported) return;
    if (pref !== "on") return;
    if (!isMobile) return;
    if (getDocumentFsElement()) return;
    void requestFs().catch(() => {
      /* ignore — usually a user-gesture requirement */
    });
  }, [isSupported, isMobile, pref]);

  const enter = useCallback(async () => {
    if (!isSupported) return;
    try {
      await requestFs();
      setPref("on");
      writePref("on");
    } catch {
      // ignore
    }
  }, [isSupported]);

  const exit = useCallback(async () => {
    if (!isSupported) return;
    try {
      await exitFs();
      setPref("off");
      writePref("off");
    } catch {
      // ignore
    }
  }, [isSupported]);

  const toggle = useCallback(async () => {
    if (!isSupported) return;
    if (getDocumentFsElement()) {
      await exit();
    } else {
      await enter();
    }
  }, [isSupported, enter, exit]);

  return {
    isSupported,
    isFullscreen,
    isMobile,
    pref,
    available: isSupported && isMobile,
    enter,
    exit,
    toggle,
  };
}