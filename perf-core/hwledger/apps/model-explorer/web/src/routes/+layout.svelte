<!--
  Root layout. Mounts the dark theme + three-pane grid that all routes
  inherit. The actual three panes are composed in +page.svelte so they
  are page-scoped; the layout only owns global chrome (header, theme
  variables, fonts).
-->
<script lang="ts">
  import '../app.css';
</script>

<div class="app">
  <header class="app__header">
    <a href="/" class="app__brand">
      <span class="app__brand-mark">⌬</span>
      <span>hwLedger</span>
      <span class="app__brand-sub">model explorer</span>
    </a>
    <nav class="app__nav">
      <a href="/">Search</a>
      <a href="/about">About</a>
    </nav>
  </header>

  <main class="app__main">
    <slot />
  </main>
</div>

<style>
  :global(:root) {
    color-scheme: dark;
    --bg: #0a0c10;
    --surface-2: #11141a;
    --surface-3: #1a1f29;
    --surface-4: #232a37;
    --border: #2a2f3a;
    --fg: #e8ecf2;
    --muted: #8a93a3;
    --accent: #6ea8ff;
    --accent-soft: #1b3057;
    --error: #ff6b6b;
    --mono: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: var(--bg);
    color: var(--fg);
    font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    font-size: 14px;
    line-height: 1.5;
  }
  :global(*, *::before, *::after) { box-sizing: border-box; }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .app__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex: 0 0 auto;
  }
  .app__brand {
    display: inline-flex;
    align-items: baseline;
    gap: 0.4rem;
    font-weight: 600;
    color: var(--fg);
    text-decoration: none;
  }
  .app__brand-mark { color: var(--accent); font-size: 1.25rem; }
  .app__brand-sub { color: var(--muted); font-weight: 400; font-size: 0.8125rem; }
  .app__nav { display: flex; gap: 1rem; }
  .app__nav a {
    color: var(--muted);
    text-decoration: none;
    font-size: 0.875rem;
  }
  .app__nav a:hover { color: var(--fg); }
  .app__main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
