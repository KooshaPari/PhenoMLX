<!--
  /about — static blurb explaining what the explorer does. Kept tiny
  on purpose; the long-form docs live in the workspace README.
-->
<svelte:head>
  <title>About · hwLedger model explorer</title>
</svelte:head>

<article class="about">
  <h1>About hwLedger model explorer</h1>
  <p>
    A standalone Svelte 5 + SvelteKit UI for browsing the
    <code>hwledger</code> model index. The UI talks to a Hono proxy
    (<code>:8787</code>) which forwards calls to the Rust
    <code>hwledger-server</code> (<code>:8080</code>).
  </p>

  <h2>REST surface</h2>
  <ul>
    <li><code>POST /v1/search</code> — text + facets → ranked hits</li>
    <li><code>GET /v1/models/&lt;id&gt;</code> — model detail</li>
    <li><code>GET /v1/models/&lt;id&gt;/quants</code> — quant list</li>
    <li><code>GET /v1/models/&lt;id&gt;/similar?limit=N</code> — neighbours</li>
    <li><code>GET /v1/use-case/&lt;slug&gt;?text=&limit=</code> — curated discovery</li>
    <li><code>POST /v1/model-ask</code> — natural-language Q&amp;A</li>
    <li><code>GET /healthz</code> — liveness</li>
  </ul>

  <h2>Three-pane layout</h2>
  <p>
    <em>Facets</em> on the left narrow the search;
    <em>results</em> in the centre show scored rows;
    <em>preview</em> on the right exposes the row's payload.
  </p>

  <p class="about__back">
    <a href="/">← Back to search</a>
  </p>
</article>

<style>
  .about {
    padding: 2rem 2.5rem;
    max-width: 720px;
    margin: 0 auto;
    color: var(--fg);
  }
  h1 { font-size: 1.5rem; margin: 0 0 1rem; }
  h2 {
    font-size: 1rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin: 1.5rem 0 0.5rem;
  }
  ul { padding-left: 1.25rem; line-height: 1.7; }
  code {
    font-family: var(--mono);
    background: var(--surface-3);
    padding: 0.05rem 0.25rem;
    border-radius: 0.2rem;
  }
  .about__back a { color: var(--accent); text-decoration: none; }
</style>
