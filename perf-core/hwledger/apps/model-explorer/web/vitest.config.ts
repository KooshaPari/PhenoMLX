import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

/**
 * Vitest config — sharing the SvelteKit plugin so `$app/environment` and
 * `$lib` aliases resolve the same way they do at runtime.
 *
 * jsdom keeps us off the network while still giving the components a real
 * DOM to render against.
 */
export default defineConfig({
  plugins: [sveltekit()],
  test: {
    include: ['src/**/*.{test,spec}.ts'],
    environment: 'jsdom',
    setupFiles: ['./vitest.setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary'],
      include: ['src/**/*.{ts,svelte}'],
      exclude: ['src/**/*.test.*', 'src/routes/**/+page.svelte'],
    },
  },
});
