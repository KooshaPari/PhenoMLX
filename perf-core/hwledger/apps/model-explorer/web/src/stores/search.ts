/**
 * Search store for the Model Explorer.
 *
 * Backed by a tiny pub/sub pattern (`createSubscriber` from
 * `svelte/store`) so the three-pane layout, the search box, and the
 * discovery chips can all subscribe to a single source of truth without
 * having to thread props through every component.
 *
 * The store does not depend on Svelte 5 runes directly — it's pure TS
 * with a `subscribe()` API — so it remains easy to consume from legacy
 * `.svelte` files and from unit tests.
 */

import { writable, derived, get, type Readable, type Writable } from 'svelte/store';

import { ApiError, type ApiClient } from '$lib/api.js';
import {
  EMPTY_UI_FACETS,
  facetsFromFacets,
  facetsToFacets,
  type ResultRow,
  type SearchRequest,
  type SearchResponse,
  type UseCaseSlug,
  type UIFacetState,
} from '$lib/types.js';

/** Search status: idle / loading / error. */
export type SearchStatus = 'idle' | 'loading' | 'error';

/** Default limit for new searches. */
export const DEFAULT_LIMIT = 25;

/** Hard upper bound — matches the Hono proxy's `limit` Zod schema. */
export const MAX_LIMIT = 100;

/** Stable empty state for the result list. */
const EMPTY_RESPONSE: SearchResponse = {
  query: '',
  limit: DEFAULT_LIMIT,
  results: [],
};

export interface SearchStoreState {
  query: string;
  useCase: UseCaseSlug | null;
  facets: UIFacetState;
  limit: number;
  status: SearchStatus;
  error: string | null;
  response: SearchResponse;
  /** Monotonic counter — bumped on every successful search. */
  revision: number;
  /** Id of the row currently expanded in the preview pane. */
  selectedId: string | null;
}

const initialState: SearchStoreState = {
  query: '',
  useCase: null,
  facets: { ...EMPTY_UI_FACETS },
  limit: DEFAULT_LIMIT,
  status: 'idle',
  error: null,
  response: { ...EMPTY_RESPONSE, results: [] },
  revision: 0,
  selectedId: null,
};

export interface SearchStore {
  subscribe: Writable<SearchStoreState>['subscribe'];
  /** Imperative accessor — returns the latest snapshot synchronously. */
  snapshot(): SearchStoreState;
  /** Replace the free-text query. Triggers a debounced search. */
  setQuery(q: string): void;
  /** Apply a use-case chip — overrides the query with the chip's seed. */
  applyUseCase(slug: UseCaseSlug): void;
  /** Clear any applied use-case chip. */
  clearUseCase(): void;
  /** Patch the UI facet state and re-run the search. */
  setFacets(patch: Partial<UIFacetState>): void;
  /** Set the page-size cap. */
  setLimit(n: number): void;
  /** Cancel an in-flight search and clear errors. */
  reset(): void;
  /** Select a row by id (drives the preview pane). */
  select(id: string | null): void;
  /** Force a re-run using the current query/facets. */
  refresh(): Promise<void>;
}

/**
 * Build a SearchStore. The store reaches out to the supplied `ApiClient`
 * (which is mockable in tests) and exposes a single `subscribe()`
 * channel for Svelte components to bind against.
 */
export function createSearchStore(api: ApiClient): SearchStore {
  const inner: Writable<SearchStoreState> = writable({ ...initialState });

  /** Active request token — bumping it cancels any in-flight run. */
  let inflightToken = 0;

  function snapshot(): SearchStoreState {
    return get(inner);
  }

  async function run(): Promise<void> {
    const token = ++inflightToken;
    const state = snapshot();

    const req: SearchRequest = {
      text: state.query,
      limit: state.limit,
      sort: state.useCase ?? null,
    };
    const facets = facetsToFacets(state.facets);
    if (Object.keys(facets).length > 0) req.facets = facets;

    inner.update((s) => ({
      ...s,
      status: 'loading',
      error: null,
    }));

    try {
      // Branch: use-case shortcut → GET /v1/use-case/:slug, else search.
      const r: SearchResponse = state.useCase
        ? await api.useCase(state.useCase, state.query, state.limit)
        : await api.search(req);

      if (token !== inflightToken) return; // superseded by a newer run

      inner.update((s) => ({
        ...s,
        status: 'idle',
        error: null,
        response: r,
        revision: s.revision + 1,
      }));
    } catch (e) {
      if (token !== inflightToken) return;
      const msg = e instanceof ApiError ? `${e.status} ${e.url}` : (e as Error).message;
      inner.update((s) => ({
        ...s,
        status: 'error',
        error: msg,
        // Keep last known results so the UI doesn't flash blank.
        response: s.response,
      }));
    }
  }

  return {
    subscribe: inner.subscribe,
    snapshot,

    setQuery(q: string) {
      inner.update((s) => ({ ...s, query: q, useCase: null }));
      void run();
    },

    applyUseCase(slug: UseCaseSlug) {
      inner.update((s) => ({ ...s, useCase: slug }));
      void run();
    },

    clearUseCase() {
      inner.update((s) => ({ ...s, useCase: null }));
      void run();
    },

    setFacets(patch: Partial<UIFacetState>) {
      inner.update((s) => ({ ...s, facets: { ...s.facets, ...patch } }));
      void run();
    },

    setLimit(n: number) {
      const clamped = Math.min(Math.max(1, Math.floor(n)), MAX_LIMIT);
      inner.update((s) => ({ ...s, limit: clamped }));
      void run();
    },

    reset() {
      inflightToken++;
      inner.set({ ...initialState, facets: { ...EMPTY_UI_FACETS } });
    },

    select(id: string | null) {
      inner.update((s) => ({ ...s, selectedId: id }));
    },

    refresh: run,
  };
}

/**
 * Derived helper — returns the id→row index for the active response,
 * suitable for the preview pane when a result row is selected.
 */
export function indexById(response: SearchResponse): Map<string, ResultRow> {
  const m = new Map<string, ResultRow>();
  for (const row of response.results) m.set(row.id, row);
  return m;
}

/**
 * Derived helper — pick a row by id from the active response.
 *
 * Exposed as a Svelte `Readable` so the preview pane can subscribe to
 * `(state, rowById)` with a single `$:` statement.
 */
export function selectResult(
  store: Readable<SearchStoreState>,
): Readable<ResultRow | null> {
  return derived(store, ($s) => {
    if (!$s.selectedId) return null;
    return $s.response.results.find((r) => r.id === $s.selectedId) ?? null;
  });
}

/**
 * Convenience — typed snapshot of the current query and facets, useful
 * for syncing back to URL search params.
 */
export function urlSyncSnapshot(state: SearchStoreState): {
  q: string;
  useCase: UseCaseSlug | null;
  facets: UIFacetState;
} {
  return { q: state.query, useCase: state.useCase, facets: state.facets };
}
