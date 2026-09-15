<!--
  ResultRow — a single row in the result list (centre pane). Renders the
  model id, the fused score, and a compact one-line summary derived from
  the optional `payload` field. Clicking the row selects it (drives the
  preview pane) and navigates to the model-detail route.
-->
<script lang="ts">
  import type { ResultRow as ResultRowT } from '$lib/types.js';

  export let row: ResultRowT;
  export let selected = false;
  export let rank = 0;

  function onClick() {
    // We dispatch via a custom event so the parent list can decide
    // whether to call `store.select(...)` or `goto(...)`.
  }

  $: facets = row.facets ?? {};
  $: kinds = facets.kinds ?? [];
  $: modalities = facets.modalities ?? [];
  $: quants = facets.quants ?? [];
  $: summary =
    (kinds[0] ?? '') +
    (kinds.length && modalities.length ? ' · ' : '') +
    (modalities[0] ?? '');
  $: id = row.id ?? '';
</script>

<a
  class="result-row"
  class:result-row--selected={selected}
  href={`/models/${encodeURIComponent(id)}`}
  data-testid="result-row"
  data-id={id}
  data-rank={rank}
  on:click
>
  <span class="result-row__rank">#{rank}</span>
  <div class="result-row__main">
    <div class="result-row__id" title={id}>{id}</div>
    <div class="result-row__meta">
      {#if summary}<span>{summary}</span>{/if}
      {#if quants.length}
        <span class="result-row__quants" aria-label="Quantizations">
          {quants.slice(0, 3).join(', ')}{quants.length > 3 ? ', …' : ''}
        </span>
      {/if}
    </div>
  </div>
  <div class="result-row__score" aria-label={`Score ${row.score.toFixed(3)}`}>
    {row.score.toFixed(3)}
  </div>
</a>

<style>
  .result-row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 0.75rem;
    align-items: center;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border, #2a2f3a);
    color: var(--fg, #e8ecf2);
    text-decoration: none;
    cursor: pointer;
  }
  .result-row:hover { background: var(--surface-3, #1a1f29); }
  .result-row--selected { background: var(--accent-soft, #1b3057); }
  .result-row__rank {
    font-variant-numeric: tabular-nums;
    color: var(--muted, #8a93a3);
    font-size: 0.75rem;
  }
  .result-row__main { min-width: 0; }
  .result-row__id {
    font-family: var(--mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.875rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result-row__meta {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: var(--muted, #8a93a3);
    margin-top: 0.125rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result-row__score {
    font-variant-numeric: tabular-nums;
    font-size: 0.75rem;
    color: var(--accent, #6ea8ff);
  }
</style>
