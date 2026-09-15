<!--
  FacetSidebar — left-most pane. Renders three facet groups
  (kind / modality / quant) plus a license free-text filter and a numeric
  parameter-count range. Each checkbox toggle rewrites the
  SearchStoreState which in turn triggers a debounced re-search.
-->
<script lang="ts">
  import type { SearchStore, SearchStoreState } from '$stores/search.js';
  import { EMPTY_UI_FACETS, type UIFacetState } from '$lib/types.js';

  export let store: SearchStore;

  /** Common taxonomy options. The list is intentionally static — the
   *  proxy will validate / ignore unknown values, but a closed list
   *  keeps the UX predictable. */
  const KIND_OPTIONS = [
    'base',
    'instruct',
    'chat',
    'reasoning',
    'embedding',
    'reranker',
    'code',
  ];
  const MODALITY_OPTIONS = ['text', 'code', 'vision', 'audio', 'image', 'multimodal'];
  const QUANT_OPTIONS = ['fp16', 'fp32', 'q8_0', 'q4_k_m', 'q5_k_m', 'awq-int4', 'gptq-int4'];

  $: state = (($store as unknown) as SearchStoreState) ?? ({} as SearchStoreState);
  $: facets = state.facets ?? { ...EMPTY_UI_FACETS };

  function toggle(list: 'kinds' | 'modalities' | 'quants', value: string) {
    const current = facets[list] ?? [];
    const next = current.includes(value)
      ? current.filter((v) => v !== value)
      : [...current, value];
    store.setFacets({ [list]: next } as Partial<UIFacetState>);
  }

  function onParamChange(field: 'minParams' | 'maxParams', ev: Event) {
    const target = ev.target as HTMLInputElement;
    store.setFacets({ [field]: target.value } as Partial<UIFacetState>);
  }

  function onLicense(ev: Event) {
    const target = ev.target as HTMLInputElement;
    store.setFacets({ license: target.value } as Partial<UIFacetState>);
  }

  function reset() {
    store.setFacets({ ...EMPTY_UI_FACETS });
  }
</script>

<aside class="facet-sidebar" aria-label="Search facets">
  <header class="facet-sidebar__head">
    <h2>Filters</h2>
    <button class="link" type="button" on:click={reset}>Reset</button>
  </header>

  <section class="facet-group">
    <h3>Kind</h3>
    {#each KIND_OPTIONS as opt (opt)}
      <label class="checkbox">
        <input
          type="checkbox"
          checked={facets.kinds?.includes(opt) ?? false}
          on:change={() => toggle('kinds', opt)}
        />
        <span>{opt}</span>
      </label>
    {/each}
  </section>

  <section class="facet-group">
    <h3>Modality</h3>
    {#each MODALITY_OPTIONS as opt (opt)}
      <label class="checkbox">
        <input
          type="checkbox"
          checked={facets.modalities?.includes(opt) ?? false}
          on:change={() => toggle('modalities', opt)}
        />
        <span>{opt}</span>
      </label>
    {/each}
  </section>

  <section class="facet-group">
    <h3>Quant</h3>
    {#each QUANT_OPTIONS as opt (opt)}
      <label class="checkbox">
        <input
          type="checkbox"
          checked={facets.quants?.includes(opt) ?? false}
          on:change={() => toggle('quants', opt)}
        />
        <span>{opt}</span>
      </label>
    {/each}
  </section>

  <section class="facet-group">
    <h3>Parameter count (B)</h3>
    <div class="param-range">
      <label>
        <span>min</span>
        <input
          type="number"
          inputmode="numeric"
          min="0"
          step="0.1"
          value={facets.minParams ?? ''}
          on:input={(e) => onParamChange('minParams', e)}
        />
      </label>
      <label>
        <span>max</span>
        <input
          type="number"
          inputmode="numeric"
          min="0"
          step="0.1"
          value={facets.maxParams ?? ''}
          on:input={(e) => onParamChange('maxParams', e)}
        />
      </label>
    </div>
  </section>

  <section class="facet-group">
    <h3>License</h3>
    <input
      type="text"
      placeholder="apache-2.0, mit, …"
      value={facets.license ?? ''}
      on:input={onLicense}
    />
  </section>
</aside>

<style>
  .facet-sidebar {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1rem;
    background: var(--surface-2, #11141a);
    border-right: 1px solid var(--border, #2a2f3a);
    overflow-y: auto;
    width: 220px;
    flex: 0 0 220px;
  }
  .facet-sidebar__head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  h2 { font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.05em; margin: 0; }
  h3 {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted, #8a93a3);
    margin: 0 0 0.375rem 0;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent, #6ea8ff);
    cursor: pointer;
    font: inherit;
    padding: 0;
  }
  .facet-group { display: flex; flex-direction: column; gap: 0.25rem; }
  .checkbox { display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; }
  .param-range { display: flex; gap: 0.5rem; }
  .param-range label { display: flex; flex-direction: column; flex: 1; font-size: 0.75rem; color: var(--muted, #8a93a3); }
  .param-range input, input[type="text"] {
    width: 100%;
    padding: 0.3rem 0.4rem;
    background: var(--surface-3, #1a1f29);
    color: var(--fg, #e8ecf2);
    border: 1px solid var(--border, #2a2f3a);
    border-radius: 0.25rem;
    font: inherit;
    box-sizing: border-box;
  }
</style>
