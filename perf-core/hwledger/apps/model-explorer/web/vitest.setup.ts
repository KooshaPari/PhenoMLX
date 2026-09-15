/**
 * Vitest setup.
 *
 * Centralised in one file so we never sprinkle jsdom / fetch polyfills
 * across the test files. The env mocks are intentionally coarse — they
 * only need to keep SvelteKit's own imports from crashing on load.
 */

import { vi } from 'vitest';

// jsdom doesn't ship $app/* modules; mock them just enough for the
// generic component tests below.
vi.mock('$app/environment', () => ({
  browser: true,
  dev: true,
  building: false,
  version: 'test',
}));

vi.mock('$app/navigation', () => ({
  goto: vi.fn(),
  invalidate: vi.fn(),
  invalidateAll: vi.fn(),
  afterNavigate: vi.fn(),
  beforeNavigate: vi.fn(),
  onNavigate: vi.fn(),
  pushState: vi.fn(),
  replaceState: vi.fn(),
}));

if (typeof globalThis.fetch !== 'function') {
  // jsdom omits fetch; the API tests stub it per-test anyway, but provide
  // a noop default so unrelated imports don't blow up.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as unknown as { fetch: typeof fetch }).fetch = (() =>
    Promise.resolve(new Response())) as unknown as typeof fetch;
}
