import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_'],
  build: {
    assetsInlineLimit: 0,
    target: 'safari15',
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        companion: fileURLToPath(new URL('./companion.html', import.meta.url)),
        companionPreview: fileURLToPath(
          new URL('./companion-preview.html', import.meta.url),
        ),
      },
    },
  },
});
