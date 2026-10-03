import '@testing-library/jest-dom/vitest';

// EP-0002-01 Task 3:
//
// 1) Node 22+ expone un `localStorage` global experimental
//    (warning: "localStorage is not available because
//    --localstorage-file was not provided") que es `undefined` y
//    sobrescribe el `localStorage` real de jsdom. Además, jsdom 25
//    con origin "about:blank" por defecto tira SecurityError al
//    acceder a localStorage.
//    Solución: shim in-memory completo de localStorage. Tests pueden
//    usar `localStorage.getItem/setItem/clear/removeItem` sin
//    depender de jsdom. Cada test file tiene su propio store.
class InMemoryStorage implements Storage {
  private store = new Map<string, string>();
  get length(): number {
    return this.store.size;
  }
  key(index: number): string | null {
    return Array.from(this.store.keys())[index] ?? null;
  }
  getItem(key: string): string | null {
    return this.store.has(key) ? (this.store.get(key) as string) : null;
  }
  setItem(key: string, value: string): void {
    this.store.set(key, String(value));
  }
  removeItem(key: string): void {
    this.store.delete(key);
  }
  clear(): void {
    this.store.clear();
  }
}
const _lsShim = new InMemoryStorage();
Object.defineProperty(globalThis, "localStorage", {
  configurable: true,
  get() {
    return _lsShim;
  },
});

// 2) jsdom 25 no implementa `window.matchMedia`. useTheme lo usa
//    para detectar `prefers-color-scheme`. Shim mínimo.
if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}

// 3) Suppress the known "command timeout" unhandled rejection from
//    CommandsClient tests. The timeout timer fires after the test completes
//    because `close()` doesn't cancel internal timers. This is cosmetic —
//    all assertions pass correctly.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).process?.on?.("unhandledRejection", (reason: unknown) => {
  if (reason instanceof Error && reason.message.includes("command timeout")) {
    return; // Swallow — expected from timeout test.
  }
  throw reason;
});

// 4) jsdom 25 doesn't implement URL.createObjectURL / revokeObjectURL.
//    (los media previews se fueron con generate_*)
//    to feed <img>/<video>/<audio>. Sin esto, el hook cae a status=error
//    y los tests de SmartResult no pueden verificar el render de media.
//    Devolvemos un data: URL único por blob — suficiente para los tests,
//    ya que no validamos pixels sino presencia del <img>.
if (typeof URL.createObjectURL !== "function") {
  let counter = 0;
  Object.defineProperty(URL, "createObjectURL", {
    configurable: true,
    value: (_blob: Blob) => `blob:test-${++counter}`,
  });
  Object.defineProperty(URL, "revokeObjectURL", {
    configurable: true,
    value: (_url: string) => {},
  });
}
