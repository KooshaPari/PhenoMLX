<!--
  /  — search landing page. Three-pane layout:
    ┌────────────┬────────────────────────────┬──────────────┐
    │  Facets    │   Search bar / discovery   │  Preview     │
    │            │   Results                  │  pane        │
    └────────────┴────────────────────────────┴──────────────┘
-->
<script lang="ts">
  import { api, ApiError } from '$lib/api.js';
  import { createSearchStore } from '$stores/search.js';
  import FacetSidebar from '$components/FacetSidebar.svelte';
  import DiscoveryBar from '$components/DiscoveryBar.svelte';
  import ResultRow from '$components/ResultRow.svelte';
  import PreviewPane from '$components/PreviewPane.svelte';
  import { onMount } from 'svelte';

  /** Module-scoped store. Created once per browser tab. */
  const store = createSearchStore(api);

  /** Bound text input. */
  let queryInput = '';
  let submitTimer: ReturnType<typeof setTimeout> | undefined;

  function onSearchInput(ev: Event) {
    const t = ev.target as HTMLInputElement;
    queryInput = t.value;
    if (submitTimer) clearTimeout(submitTimer);
    submitTimer = setTimeout(() => store.setQuery(queryInput.trim()), 220);
  }

  function onSearchSubmit(ev: Event) {
    ev.preventDefault();
    if (submitTimer) clearTimeout(submitTimer);
    store.setQuery(queryInput.trim());
  }

  function onRowActivate(ev: CustomEvent<{ id: string }>) {
    const id = ev.detail?.id;
    if (id) store.select(id);
  }

  // Auto-select the first row on every fresh search so the preview pane
  // is never empty while the user is exploring.
  let lastRevision = -1;
  $: if ($store && $store.revision !== lastRevision) {
    lastRevision = $store.revision;
    if (!$store.selectedId && $store.response.results.length > 0) {
      store.select($store.response.results[0].id);
    }
  }

  // --- keyboard navigation ---
  let searchInputEl: HTMLInputElement;
  let resultEls: HTMLOListElement;

  function onKeydown(ev: KeyboardEvent) {
    const results = $store.response.results;
    const idx = $store.selectedId ? results.findIndex((r) => r.id === $store.selectedId) : -1;

    if (ev.key === '/' && ev.target !== searchInputEl) {
      ev.preventDefault();
      searchInputEl?.focus();
      searchInputEl?.select();
      return;
    }
    if (ev.key === 'Escape') {
      store.select(null);
      searchInputEl?.blur();
      return;
    }
    if (ev.key === 'ArrowDown' || ev.key === 'j') {
      ev.preventDefault();
      const next = Math.min(idx + 1, results.length - 1);
      if (results[next]) store.select(results[next].id);
      return;
    }
    if (ev.key === 'ArrowUp' || ev.key === 'k') {
      ev.preventDefault();
      const prev = Math.max(idx - 1, 0);
      if (results[prev]) store.select(results[prev].id);
      return;
    }
    if (ev.key === 'Enter' && !ev.repeat && $store.selectedId) {
      // Navigate to the detail page unless already on a form
      if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLButtonElement) return;
      ev.preventDefault();
      window.location.href = `/models/${encodeURIComponent($store.selectedId)}`;
    }
  }

  onMount(() => {
    // Kick off an empty search so the page isn't blank.
    store.refresh();
    return () => {
      if (submitTimer) clearTimeout(submitTimer);
    };
  });
</script>

<svelte:head>
  <title>Search · hwLedger model explorer</title>
</svelte:head>

<svelte:window on:keydown={onKeydown} />

<div class="three-pane" data-busy={$store.status === 'loading'}>
  <FacetSidebar {store} />

  <section class="centre" aria-label="Search">
    <form class="search-bar" on:submit={onSearchSubmit} role="search">
      <input
        type="search"
        autocomplete="off"
        spellcheck="false"
        placeholder="Search models — e.g. ‘llama 8b instruct’, ‘qwen2 coder’, ‘bge embedding’"
        aria-label="Search models (press / to focus)"
        data-testid="search-input"
        value={queryInput}
        on:input={onSearchInput}
        bind:this={searchInputEl}
      />
      <button type="submit" data-testid="search-submit">Search</button>
    </form>

    <DiscoveryBar active={$store.useCase} busy={$store.status === 'loading'} />

    <div class="results" data-testid="results">
      {#if $store.status === 'loading' && $store.response.results.length === 0}
        <div class="results__state">Searching…</div>
      {:else if $store.error}
        <div class="results__state results__state--error" role="alert">
          Error: {$store.error}
        </div>
      {:else if $store.response.results.length === 0}
        <div class="results__state">No results.</div>
      {:else}
        <ol class="results__list" bind:this={resultEls}>
          {#each $store.response.results as row, i (row.id)}
            <li>
              <ResultRow
                {row}
                rank={i + 1}
                selected={$store.selectedId === row.id}
                on:click={() => store.select(row.id)}
              />
            </li>
          {/each}
        </ol>
      {/if}
    </div>
  </section>

  <PreviewPane
    row={$store.response.results.find((r) => r.id === $store.selectedId) ?? null}
    loading={$store.status === 'loading' && $store.selectedId === null}
    error={$store.error}
  />
</div>

<style>
  .three-pane {
    display: grid;
    grid-template-columns: 220px 1fr 380px;
    height: 100%;
    min-height: 0;
  }
  .centre {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-right: 1px solid var(--border);
  }
  .search-bar {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .search-bar input {
    flex: 1;
    padding: 0.5rem 0.75rem;
    background: var(--surface-3);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    font: inherit;
  }
  .search-bar input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .search-bar button {
    padding: 0.5rem 1rem;
    background: var(--accent);
    color: #0a0c10;
    border: none;
    border-radius: 0.4rem;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .results { flex: 1; overflow-y: auto; min-height: 0; }
  .results__state {
    padding: 1.5rem;
    text-align: center;
    color: var(--muted);
  }
  .results__state--error { color: var(--error); }
  .results__list { list-style: none; margin: 0; padding: 0; }
</style>
