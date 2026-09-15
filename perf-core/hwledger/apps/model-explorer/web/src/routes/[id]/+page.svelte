<!--
  /models/[id] — single-model detail view.

  Uses SvelteKit's universal `load` function so the same fetch happens
  during SSR and during client-side navigation. We surface the
  `/v1/models/:id` and `/v1/models/:id/quants` endpoints and provide a
  small "similar models" rail that calls `/v1/models/:id/similar`.
-->
<script lang="ts">
  import { api, ApiError } from '$lib/api.js';
  import { onMount } from 'svelte';
  import type {
    ModelDetail,
    QuantsResponse,
    SimilarResponse,
  } from '$lib/types.js';

  /** Provided by `+page.ts` via `data`. */
  export let data: { id: string };

  let detail: ModelDetail | null = null;
  let quants: QuantsResponse | null = null;
  let similar: SimilarResponse | null = null;
  let loading = true;
  let error: string | null = null;

  onMount(async () => {
    loading = true;
    error = null;
    try {
      const [d, q, s] = await Promise.all([
        api.detail(data.id),
        api.quants(data.id).catch(() => null as unknown as QuantsResponse),
        api.similar(data.id, 5).catch(() => null as unknown as SimilarResponse),
      ]);
      detail = d;
      quants = q ?? { id: data.id, quants: [] };
      similar = s ?? { id: data.id, results: [] };
    } catch (e) {
      error = e instanceof ApiError ? `${e.status} ${e.url}` : (e as Error).message;
    } finally {
      loading = false;
    }
  });
</script>

<svelte:head>
  <title>{data.id} · hwLedger model explorer</title>
</svelte:head>

<article class="detail" aria-label="Model detail">
  <a class="detail__back" href="/">← Back to search</a>

  {#if loading}
    <div class="detail__state" role="status">Loading…</div>
  {:else if error}
    <div class="detail__state detail__state--error" role="alert">{error}</div>
  {:else if !detail || !detail.found}
    <div class="detail__state">
      <h2>Not found</h2>
      <p>No model with id <code>{data.id}</code> exists in the index.</p>
    </div>
  {:else}
    <header class="detail__head">
      <h1>{detail.id}</h1>
      <dl class="detail__summary">
        {#if detail.kind}<div><dt>Kind</dt><dd>{detail.kind}</dd></div>{/if}
        {#if detail.score !== null && detail.score !== undefined}
          <div><dt>Score</dt><dd>{detail.score.toFixed(4)}</dd></div>
        {/if}
        <div><dt>Found</dt><dd>{detail.found ? 'yes' : 'no'}</dd></div>
      </dl>
    </header>

    {#if quants && quants.quants.length}
      <section class="detail__section">
        <h2>Quants</h2>
        <ul class="detail__quants">
          {#each quants.quants as q (q)}
            <li>{q}</li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if similar && similar.results.length}
      <section class="detail__section">
        <h2>Similar</h2>
        <ol class="detail__similar">
          {#each similar.results as row, i (row.id)}
            <li>
              <a href={`/models/${encodeURIComponent(row.id)}`}>
                <span class="detail__similar-rank">#{i + 1}</span>
                <span class="detail__similar-id">{row.id}</span>
                <span class="detail__similar-score">{row.score.toFixed(3)}</span>
              </a>
            </li>
          {/each}
        </ol>
      </section>
    {/if}
  {/if}
</article>

<style>
  .detail {
    padding: 1.5rem 2rem;
    max-width: 960px;
    margin: 0 auto;
    width: 100%;
  }
  .detail__back {
    display: inline-block;
    margin-bottom: 1rem;
    color: var(--accent);
    text-decoration: none;
    font-size: 0.875rem;
  }
  .detail__back:hover { text-decoration: underline; }
  .detail__head h1 {
    font-family: var(--mono);
    font-size: 1.25rem;
    margin: 0 0 0.75rem;
    word-break: break-all;
  }
  .detail__summary {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 1rem;
    margin: 0 0 1.5rem;
    font-size: 0.875rem;
  }
  .detail__summary > div { display: contents; }
  dt { color: var(--muted); }
  dd { margin: 0; }
  .detail__section { margin: 1.5rem 0; }
  .detail__section h2 {
    font-size: 0.875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin: 0 0 0.5rem;
  }
  .detail__quants {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .detail__quants li {
    padding: 0.2rem 0.5rem;
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    font-family: var(--mono);
    font-size: 0.75rem;
  }
  .detail__similar {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .detail__similar a {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 0.75rem;
    padding: 0.4rem 0.6rem;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    color: var(--fg);
    text-decoration: none;
    align-items: center;
  }
  .detail__similar a:hover { background: var(--surface-3); }
  .detail__similar-rank {
    color: var(--muted);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }
  .detail__similar-id {
    font-family: var(--mono);
    font-size: 0.8125rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail__similar-score {
    color: var(--accent);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
  }
  .detail__state { padding: 1rem; color: var(--muted); }
  .detail__state--error { color: var(--error); }
  code {
    font-family: var(--mono);
    background: var(--surface-3);
    padding: 0.05rem 0.25rem;
    border-radius: 0.2rem;
  }
</style>
