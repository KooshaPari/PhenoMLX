<!--
  PreviewPane — right-most pane. Renders the selected row's payload as
  a syntax-highlighted-ish JSON view, plus a header with the row id and
  score. Falls back to an empty-state placeholder when nothing is
  selected.
-->
<script lang="ts">
  import type { ResultRow as ResultRowT } from '$lib/types.js';

  export let row: ResultRowT | null = null;
  export let loading = false;
  export let error: string | null = null;

  /** Compact JSON formatter with stable indentation. */
  function safeStringify(v: unknown): string {
    try {
      return JSON.stringify(v, null, 2);
    } catch {
      return String(v);
    }
  }

  $: payload = row?.payload ?? null;
  $: payloadText = payload === null ? '' : safeStringify(payload);
  $: facets = row?.facets ?? {};
</script>

<section class="preview-pane" aria-label="Preview">
  <header class="preview-pane__head">
    <h2>Preview</h2>
    {#if row}
      <span class="preview-pane__id" title={row.id}>{row.id}</span>
    {/if}
  </header>

  {#if loading}
    <div class="preview-pane__state" role="status">Loading…</div>
  {:else if error}
    <div class="preview-pane__state preview-pane__state--error" role="alert">
      {error}
    </div>
  {:else if !row}
    <div class="preview-pane__state">
      <p>Select a model to inspect its payload here.</p>
      <p class="muted">
        Use the search bar above, or click one of the discovery chips
        (coding / agentic / reasoning / embedding) to get started.
      </p>
    </div>
  {:else}
    <dl class="preview-pane__summary">
      <div><dt>Score</dt><dd>{row.score.toFixed(4)}</dd></div>
      {#if facets.kinds?.length}
        <div><dt>Kind</dt><dd>{facets.kinds.join(', ')}</dd></div>
      {/if}
      {#if facets.modalities?.length}
        <div><dt>Modality</dt><dd>{facets.modalities.join(', ')}</dd></div>
      {/if}
      {#if facets.attention_kinds?.length}
        <div><dt>Attention</dt><dd>{facets.attention_kinds.join(', ')}</dd></div>
      {/if}
      {#if facets.quants?.length}
        <div><dt>Quants</dt><dd>{facets.quants.join(', ')}</dd></div>
      {/if}
      {#if facets.license}
        <div><dt>License</dt><dd>{facets.license}</dd></div>
      {/if}
    </dl>

    {#if payloadText}
      <pre class="preview-pane__payload" data-testid="preview-payload">{payloadText}</pre>
    {/if}
  {/if}
</section>

<style>
  .preview-pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-2, #11141a);
    border-left: 1px solid var(--border, #2a2f3a);
    overflow: hidden;
  }
  .preview-pane__head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border, #2a2f3a);
  }
  h2 { font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.05em; margin: 0; }
  .preview-pane__id {
    font-family: var(--mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.75rem;
    color: var(--muted, #8a93a3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 60%;
  }
  .preview-pane__state {
    padding: 1rem;
    color: var(--muted, #8a93a3);
    font-size: 0.875rem;
  }
  .preview-pane__state--error { color: var(--error, #ff6b6b); }
  .preview-pane__summary {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 0.25rem 0.75rem;
    padding: 0.75rem;
    margin: 0;
    font-size: 0.8125rem;
  }
  .preview-pane__summary > div { display: contents; }
  dt { color: var(--muted, #8a93a3); }
  dd { margin: 0; }
  .preview-pane__payload {
    margin: 0;
    padding: 0.75rem;
    background: var(--surface-3, #1a1f29);
    border-top: 1px solid var(--border, #2a2f3a);
    overflow: auto;
    flex: 1;
    font-family: var(--mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.75rem;
    line-height: 1.45;
  }
  .muted { color: var(--muted, #8a93a3); }
</style>
