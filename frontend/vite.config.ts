import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '0.0.0.0',
    port: 1420,
    strictPort: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:4040',
        changeOrigin: true,
      },
      '/chat': {
        target: 'http://127.0.0.1:4040',
        changeOrigin: true,
      },
      '/rpc': {
        target: 'http://127.0.0.1:4040',
        changeOrigin: true,
      },
    },
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
}));
