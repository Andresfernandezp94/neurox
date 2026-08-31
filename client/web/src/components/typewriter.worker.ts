/// <reference lib="webworker" />
export {};

/**
 * Web Worker that drives the typewriter animation off the main thread.
 * Receives text updates via postMessage and posts back the current
 * "revealed" length at ~60fps. The main thread only applies text.slice().
 */

// EP-0026-rev-fix: typewriter delay removed. The worker reveals the
// full text on the next animation frame after each update — no
// per-char rate-limiting. The caret blink on the main thread still
// signals "this is a live stream" visually.

let text = "";
let revealed = 0;
let rafId = 0;

function tick(): void {
  if (revealed < text.length) {
    revealed = text.length;
    self.postMessage({ type: "reveal", revealed });
  }
  rafId = 0;
}

self.addEventListener("message", (e: MessageEvent) => {
  const msg = e.data;
  if (msg.type === "update") {
    text = msg.text;
    if (revealed > text.length) revealed = text.length;
    if (revealed < text.length && rafId === 0) {
      rafId = self.requestAnimationFrame(tick);
    } else if (revealed === text.length && rafId === 0) {
      // No-op: already caught up.
    } else if (revealed === text.length) {
      self.postMessage({ type: "reveal", revealed });
    }
  } else if (msg.type === "reset") {
    text = "";
    revealed = 0;
    if (rafId) {
      self.cancelAnimationFrame(rafId);
      rafId = 0;
    }
  }
});
