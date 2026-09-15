import { defineConfig } from 'vitest/config';

/**
 * Vitest config for the Hono proxy server.
 *
 * Coverage is intentionally local-only (`src/**`) and we pin the test env to
 * `node` because the proxy invokes `fetch`, which needs the real Node runtime
 * — happy-dom / jsdom would silently fall through and break network stubbing.
 */
export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.ts'],
    environment: 'node',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary'],
      include: ['src/**/*.ts'],
      exclude: ['src/**/__tests__/**'],
    },
  },
});
