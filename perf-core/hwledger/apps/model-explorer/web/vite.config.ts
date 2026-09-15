import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

/**
 * Vite configuration for the SvelteKit UI.
 *
 * We pin Vite 6.3 because that's the floor `@sveltejs/vite-plugin-svelte@6`
 * supports (peer = `^6.3.0 || ^7.0.0`); vite-plugin-svelte 6 sidesteps the
 * `esrap 1.4.9` bug that breaks `vite-plugin-svelte@4` during build.
 *
 * Env: `PUBLIC_MODEL_EXPLORER_API` can override the API base URL at build
 * time. Defaults to the local Hono proxy on `:8787`.
 */
export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 5173,
    strictPort: false,
    proxy: {
      // Forward API calls during dev to bypass CORS.
      '/v1': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: true,
      },
      '/healthz': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: true,
      },
    },
  },
});
