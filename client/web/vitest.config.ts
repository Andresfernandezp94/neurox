import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Config separado de vitest (EP-0001-02).
// Separado del vite.config.ts para evitar mezclar tipos de UserConfig
// (Vite) con los de vitest (que agrega `test`, `coverage`, etc).
//
// EP-0002-01 Task 3: agregamos `url` a environmentOptions.jsdom
// porque jsdom 25+ requiere un origin real (no opaque/blank) para
// que `localStorage` esté disponible. Sin esto, los tests que
// usan localStorage (ej. useTheme) fallan con
// "Cannot read properties of undefined (reading 'clear')".
export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    environmentOptions: {
      jsdom: {
        url: 'http://localhost:5173',
      },
    },
    setupFiles: ['./src/test-setup.ts'],
    css: false,
  },
});
