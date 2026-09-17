# HFScope Chrome Extension

A thin Chrome / Edge / Brave / Arc / Opera extension that wraps your
self-hosted HFScope server and exposes it on every `huggingface.co` page.

## What it does

| Mode | Trigger | What you see |
|---|---|---|
| **Side panel** | `Alt+H` (or click the toolbar icon on a hub page) | A persistent right-side pane showing HFScope results filtered by the page you're on (model page = that model in a compare view; search results page = same query in HFScope's filter UI) |
| **Quick search popup** | Click the toolbar icon, or `⌘/Ctrl-Shift-K` | A small popup with a search box + type filter (models / spaces / datasets); Enter opens `/results` in a new tab |
| **Page-level "compare" button** | Auto-injected on every model card in the HF catalog | Click the `⇄` chip on any model card to add it to a 4-slot compare queue |

## Architecture

```
                ┌──────────────────────────────┐
   HF catalog  │  /opt/hfscope-webview        │  Alt+H side panel
  huggingface  │  ────────────────            │   shows:
   .co page     │   side-panel.html (iframe)   │   /embed/results?q=…
                │         \_______________     │   /embed/model/{id}
                └─────────────┼────────────────┘
                              ▼
              ┌────────────────────────────────────┐
              │ http://localhost:9190             │
              │  HFScope Go server                 │
              │  ─ /embed/results                 │
              │  ─ /embed/model/{id}              │
              │  ─ /feed.rss                       │
              │  ─ /static/embed.js                │
              └────────────────────────────────────┘
```

The extension is **a thin launcher, not a separate app**. Every pixel of UI
comes from the HFScope server. This means:

- Update the server, get the new UI in the extension automatically
- One place to test, one place to ship features
- No per-platform code (the same extension runs on Mac/Win/Linux Chromium)

## Install (development)

1. Run your HFScope server:
   ```bash
   cd /Users/<REDACTED>/CodeProjects/Phenotype/repos/hfscope
   HFSCOPE_ADDR=:9190 ./bin/hfscope
   ```

2. Open Chrome → `chrome://extensions/` → enable **Developer mode** (top right).

3. Click **Load unpacked** → select this directory.

4. Click the puzzle-piece icon → pin **HFScope**.

5. Visit `https://huggingface.co/models?other=llama` and press `Alt+H` —
   the side panel opens.

## Bundle for production

The extension is plain HTML/TS/CSS — no build step is strictly required
if you want to ship the source as-is. But for a production release you'll
want to:
1. `tsc background.ts content.ts` (or use esbuild/Vite)
2. Inline `side-panel.html` with the bundled `side-panel.js`
3. Generate PNG icons in `icons/` (current directory ships placeholder PNGs)
4. ZIP the directory and upload to the [Chrome Web Store dashboard](https://chrome.google.com/webstore/devconsole/)

## Files

| File | Purpose |
|---|---|
| `manifest.json` | MV3 manifest: permissions, content scripts, commands, icons |
| `background.ts` | Service worker. Opens the side panel on `Alt+H`; routes shortcuts to popup/iframe |
| `side-panel.html` + `side-panel.js` | The persistent right pane UI (loads `http://localhost:9190/embed/results` in an iframe) |
| `popup.html` + `popup.js` | 380px quick-search popup |
| `content.ts` | Injected on HF catalog pages — adds `⇄` compare chip to model cards and a FAB on model detail pages |
| `options.html` + `options.js` | Config page for the server URL |
| `icons/16,32,48,128.png` | Toolbar + store listing icons |

## What the extension needs from the HFScope server

These endpoints are already implemented or on the Tier 3.4 roadmap:

- `GET /embed/results?q=…&type=models|spaces|datasets&limit=…` — fragment-only HTML
- `GET /embed/model/{id}` — single-model fragment
- `GET /embed.js` — tiny JS embed helper for cross-origin

All must set `Access-Control-Allow-Origin: *`. (Already supported by
`/feed.rss`; will be added to `/embed/*` in Tier 3.4.)

## Configurable

Click the toolbar icon → **Manage extension** → **Extension options**, or
right-click the toolbar icon → **Options**. Set the URL of your HFScope
server (defaults to `http://localhost:9190`).

Stored in `chrome.storage.local` under the key `hfscopeHost`.

## Ship the side panel right

The side panel is the killer feature. Pinning a single `Alt+H` opens
HFScope next to **any** HF page. Build the rest of the modes as polish;
ship the panel first.
