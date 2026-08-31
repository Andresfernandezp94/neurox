import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { proxyErrorGuard } from './vite-plugins/proxy-error-guard';

// EP-0001-02: Vite dev server con proxy al daemon neurox en :7878.
// En producción, el bundle es servido por el daemon directamente desde
// static_dir, no por Vite.
//
// changeOrigin: true reescribe el Host header al del target (127.0.0.1:7878).
// Eso evita problemas cuando el cliente llega por LAN (192.168.1.5:5173)
// y el daemon valida el Host. Sin esto, algunas rutas devuelven 404 o
// el WebSocket upgrade se rechaza.
export default defineConfig({
  plugins: [react(), proxyErrorGuard()],
  server: {
    port: 5173,
    host: true,
    proxy: {
      // WebSocket para /v1/events — DEBE ir antes que /v1 para que
      // http-proxy no matchee el prefijo /v1 antes y pierda el
      // upgrade WS. Bug visto en 2026-08-12: SPA en navegador abría
      // el WS y se cerraba antes de establecerse.
      '/v1/events': {
        target: 'ws://127.0.0.1:7878',
        ws: true,
        changeOrigin: true,
      },
      // EP-0002: WebSocket para el voice MCP (`mcps/voice/voiced`,
      // puerto 9998). Mismo patrón que /v1/events — DEBE ir antes que
      // cualquier regla `/voice*` más amplia.
      '/voice/ws': {
        target: 'ws://127.0.0.1:9998',
        ws: true,
        changeOrigin: true,
      },
      // EP-0002: REST del voice MCP (POST /voice/start, /voice/end,
      // /voice/speak, …). En producción estos endpoints deberían
      // estar expuestos por el daemon en `/v1/voice/*` con forward al
      // plugin; mientras tanto, en dev Vite los redirige directo.
      '/voice': {
        target: 'http://127.0.0.1:9998',
        changeOrigin: true,
      },
      // API REST
      '/v1': {
        target: 'http://127.0.0.1:7878',
        changeOrigin: true,
      },
      '/health': {
        target: 'http://127.0.0.1:7878',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
  },
});
