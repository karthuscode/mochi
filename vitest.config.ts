import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: 'domain',
          environment: 'node',
          include: ['packages/domain/src/**/*.test.ts'],
        },
      },
      {
        extends: true,
        test: {
          name: 'desktop',
          environment: 'jsdom',
          include: ['apps/desktop/src/**/*.test.{ts,tsx}'],
          setupFiles: ['apps/desktop/src/test/setup.ts'],
        },
      },
    ],
  },
});
