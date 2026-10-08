import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    rollupOptions: { output: { manualChunks: { markdown: ['react-markdown', 'remark-gfm'] } } },
  },
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  test: { include: ['src/**/*.test.ts'] },
});
