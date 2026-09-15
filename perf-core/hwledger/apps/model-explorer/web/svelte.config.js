import adapter from '@sveltejs/adapter-node';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      out: 'build',
      precompress: false,
      envPrefix: 'PUBLIC_',
    }),
    alias: {
      $lib: 'src/lib',
      $components: 'src/components',
      $stores: 'src/stores',
    },
  },
};

export default config;
