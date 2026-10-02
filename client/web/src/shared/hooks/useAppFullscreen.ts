// useAppFullscreen — estado de pantalla completa compartido por el shell.
//
// Por que un store a nivel de modulo y no un `useState` en cada componente:
// hay DOS botones que controlan lo mismo — el del chat footer y el de la
// barra de mobile — y viven en arboles distintos. `ChatPanel` solo esta
// montado cuando la vista activa es chat, asi que un estado local suyo no
// existe mientras se esta en Overview, Config o MCP. Con estado por
// componente, tocar el boton de la barra dejaria el icono del chat
// diciendo lo contrario (y al reentrar al chat, el estado habria quedado
// desfasado).
//
// Camino principal: Fullscreen API sobre el shell `.app`, NO sobre
// `documentElement`. Es a proposito: la sidebar vive adentro de `.app`, y
// pedirle fullscreen a `.app` la deja visible (en mobile es la bottom-bar).
// Pedirselo a `documentElement` la esconderia, que es justo lo que el
// operador pidio evitar al pedir el boton en la barra.
//
// Fallback: la clase CSS `chat-layout--focus`, para browsers sin la API
// (iOS Safari). El componente que la aplica lee `isFocusMode` de este hook.

import { useCallback, useSyncExternalStore } from "react";

type Listener = () => void;

const listeners = new Set<Listener>();

/** Fullscreen API real activa (el shell `.app` esta en pantalla completa). */
let isFullscreen = false;

/** Fallback CSS: el chat esta en `chat-layout--focus`. */
let isFocusMode = false;

let listening = false;

function emit(): void {
  for (const l of listeners) l();
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getIsFullscreen(): boolean {
  return isFullscreen;
}

function getIsFocusMode(): boolean {
  return isFocusMode;
}

/** `requestFullscreen` devuelve una promesa en la spec, pero los mocks y
 *  algunos browsers viejos devuelven `undefined`. Encerrarla a mano para no
 *  romper con `.catch` sobre `undefined`. */
function ignoreRejection(p: Promise<void> | void): void {
  if (p && typeof (p as Promise<void>).catch === "function") {
    (p as Promise<void>).catch(() => {});
  }
}

/**
 * El listener vive en el modulo, no en un efecto de componente: si lo
 * tuviera ChatPanel, no habria nadie escuchando cuando la vista no es chat,
 * y al entrar en fullscreen desde la barra el estado nunca se actualizaria.
 */
function installFullscreenListener(): void {
  if (listening) return;
  if (typeof document === "undefined") return;
  listening = true;
  document.addEventListener("fullscreenchange", () => {
    const next = Boolean(document.fullscreenElement);
    if (next === isFullscreen) return;
    isFullscreen = next;
    emit();
  });
}

/** Alterna entrar/salir de pantalla completa. Sin dependencias de React. */
export function toggleAppFullscreen(): void {
  if (typeof document === "undefined") return;
  if (document.fullscreenElement) {
    ignoreRejection(document.exitFullscreen?.());
    return;
  }
  const shell = document.querySelector<HTMLElement>(".app");
  if (shell?.requestFullscreen) {
    ignoreRejection(shell.requestFullscreen());
    return;
  }
  isFocusMode = !isFocusMode;
  emit();
}

export interface UseAppFullscreenValue {
  /** Fullscreen API real activa. */
  isFullscreen: boolean;
  /** Fallback CSS activo (`chat-layout--focus`). */
  isFocusMode: boolean;
  /** Cualquiera de los dos: lo que deben pintar los iconos. */
  isExpanded: boolean;
  toggle: () => void;
}

export function useAppFullscreen(): UseAppFullscreenValue {
  installFullscreenListener();
  const full = useSyncExternalStore(subscribe, getIsFullscreen, getIsFullscreen);
  const focus = useSyncExternalStore(subscribe, getIsFocusMode, getIsFocusMode);
  const toggle = useCallback(() => toggleAppFullscreen(), []);
  return { isFullscreen: full, isFocusMode: focus, isExpanded: full || focus, toggle };
}

/** Solo para tests: deja el modulo en estado conocido. */
export function __resetAppFullscreenForTests(): void {
  isFullscreen = false;
  isFocusMode = false;
  listeners.clear();
}
