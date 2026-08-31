// EP-0028 HMR-fix: Vite plugin that prevents the dev server from
// crashing on transient WS proxy socket errors.
//
// Why: Vite 6.4.3 attaches `proxy.on('error', ...)` but does NOT
// attach an `error` handler to the upstream socket of the WS proxy
// (`/v1/events → ws://127.0.0.1:7878`). When the daemon ECONNRESETs
// the proxy socket, or a LAN client drops mid-stream, Node emits an
// unhandled `error` event on the socket. Without a listener it
// propagates to the process and kills Vite with `ELIFECYCLE Command
// failed with exit code 1`.

import type { Plugin } from "vite";

export function proxyErrorGuard(): Plugin {
  return {
    name: "neurox:proxy-error-guard",
    apply: "serve",
    configureServer() {
      // Catch any uncaught error that escapes the upgrade hook.
      process.on("uncaughtException", (err: NodeJS.ErrnoException) => {
        if (err && (err.code === "ECONNRESET" || err.code === "EPIPE")) {
          // eslint-disable-next-line no-console
          console.warn(
            `[neurox:proxy-error-guard] swallowed ${err.code}: ${err.message}`,
          );
          return;
        }
        // eslint-disable-next-line no-console
        console.error("[neurox:proxy-error-guard] uncaughtException:", err);
        throw err;
      });
    },
  };
}
