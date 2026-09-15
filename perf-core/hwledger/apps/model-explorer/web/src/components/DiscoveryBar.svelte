<!--
  DiscoveryBar — sticky chip row above the result list. Each chip is a
  curated use-case (coding / agentic / reasoning / embedding). Clicking
  a chip sets `useCase` on the store; clicking the active chip clears it.

  Keyboard-friendly: chips are real <button>s, focus is preserved across
  re-renders, and Enter / Space activates them natively.
-->
<script lang="ts">
  import { api } from '$lib/api.js';
  import { createSearchStore } from '$stores/search.js';
  import { USE_CASES, type UseCaseSlug } from '$lib/types.js';

  /** Optional override — defaults to the module-scoped API client. */
  export let apiClient: typeof api = api;

  /** Active slug — bound to `$store.useCase`. */
  export let active: UseCaseSlug | null = null;

  /** Pending flag, surfaced via `aria-busy`. */
  export let busy = false;

  // Lazily construct a store the parent can also subscribe to. We avoid
  // instantiating it on every render by attaching to the module's
  // singleton via `getContext` in a parent layout; for the standalone
  // use-case we just create one here.
  const store = createSearchStore(apiClient);

  function handle(slug: UseCaseSlug) {
    if (active === slug) {
      active = null;
      store.clearUseCase();
    } else {
      active = slug;
      store.applyUseCase(slug);
    }
  }
</script>

<nav class="discovery-bar" aria-label="Discovery shortcuts" data-busy={busy}>
  <span class="discovery-bar__label">Try:</span>
  <ul class="discovery-bar__chips">
    {#each USE_CASES as uc (uc.slug)}
      {@const isActive = active === uc.slug}
      <li>
        <button
          type="button"
          class="chip"
          class:chip--active={isActive}
          aria-pressed={isActive}
          title={uc.description}
          on:click={() => handle(uc.slug)}
        >
          <span class="chip__label">{uc.label}</span>
          <span class="chip__hint">{uc.description}</span>
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .discovery-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    background: var(--surface-2, #11141a);
    border-bottom: 1px solid var(--border, #2a2f3a);
    overflow-x: auto;
  }
  .discovery-bar__label {
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted, #8a93a3);
    flex: 0 0 auto;
  }
  .discovery-bar__chips {
    list-style: none;
    display: flex;
    gap: 0.5rem;
    margin: 0;
    padding: 0;
  }
  .chip {
    display: inline-flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.125rem;
    padding: 0.4rem 0.75rem;
    background: var(--surface-3, #1a1f29);
    color: var(--fg, #e8ecf2);
    border: 1px solid var(--border, #2a2f3a);
    border-radius: 0.5rem;
    cursor: pointer;
    font: inherit;
    transition: background-color 120ms, border-color 120ms;
  }
  .chip:hover { background: var(--surface-4, #232a37); }
  .chip:focus-visible {
    outline: 2px solid var(--accent, #6ea8ff);
    outline-offset: 2px;
  }
  .chip--active {
    background: var(--accent-soft, #1b3057);
    border-color: var(--accent, #6ea8ff);
  }
  .chip__label {
    font-weight: 600;
    font-size: 0.875rem;
  }
  .chip__hint {
    font-size: 0.6875rem;
    color: var(--muted, #8a93a3);
  }
</style>
