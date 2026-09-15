// content.ts — page-level augmentation on huggingface.co.
// Strategy: stay out of the HF UI entirely. Just add a small "open in HFScope"
// button next to each model row in the catalog, and a floating action button
// on model detail pages that toggles the side panel.
//
// This file runs in the page context, so it has access to DOM but not chrome
// APIs directly — we use chrome.runtime.sendMessage to talk to the SW.

(function () {
  if (window.__hfscope_injected__) return;
  window.__hfscope_injected__ = true;

  const HF_HOSTS = ["huggingface.co", "www.huggingface.co"];
  if (!HF_HOSTS.includes(location.hostname)) return;

  const log = (...a) => console.log("[HFScope]", ...a);

  function getHFSCOPEHost() {
    return new Promise((res) =>
      chrome.storage.local.get(["hfscopeHost"], (s) =>
        res(s.hfscopeHost || "http://localhost:9190")
      )
    );
  }

  async function openInNewTab(url) {
    await new Promise((r) =>
      chrome.runtime.sendMessage({ kind: "openTab", url }, () => r())
    );
    // Fallback if SW handler is missing:
    window.open(url, "_blank", "noopener");
  }

  // ---- Inject "compare" button on the catalog grid ---------------------------
  function injectCatalogButtons() {
    // HF catalog uses <article> tags per model; their href starts with /<org>/<name>
    const items = document.querySelectorAll(
      "article a[href^='/'], li a[href^='/']"
    );
    items.forEach(async (a) => {
      const m = a.getAttribute("href")?.match(/^\/([^/]+\/[^/?#]+)$/);
      if (!m) return;
      if (a.querySelector(".hfscope-compare")) return;

      const btn = document.createElement("button");
      btn.className = "hfscope-compare";
      btn.title = "Add to HFScope compare";
      btn.textContent = "⇄";
      btn.style.cssText = `
        margin-left: 6px; padding: 1px 6px; font-size: 11px;
        background: #1f6feb; color: #fff; border: 0; border-radius: 4px;
        cursor: pointer; opacity: 0.6; transition: opacity 0.2s;
      `;
      btn.addEventListener("mouseenter", () => (btn.style.opacity = "1"));
      btn.addEventListener("mouseleave", () => (btn.style.opacity = "0.6"));
      btn.addEventListener("click", async (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const host = await getHFSCOPEHost();
        const stored = await chrome.storage.local.get(["compareList"]);
        const ids = (stored.compareList || []).filter((x) => x !== m[1]);
        ids.unshift(m[1]);
        if (ids.length > 4) ids.length = 4;
        await chrome.storage.local.set({ compareList: ids });
        btn.textContent = `✓ ${ids.length}`;
        chrome.runtime.sendMessage({
          kind: "toast",
          text: `Added ${m[1]} to compare (${ids.length}/4)`,
        }).catch(() => {});
      });
      a.appendChild(btn);
    });
  }

  // ---- Inject side-panel toggle FAB on model detail pages -------------------
  function injectDetailFab() {
    const path = location.pathname;
    const m = path.match(/^\/models\/([^/]+\/[^/]+)$/);
    if (!m) return;
    if (document.querySelector(".hfscope-fab")) return;

    const fab = document.createElement("button");
    fab.className = "hfscope-fab";
    fab.title = "Open HFScope side panel";
    fab.textContent = "HFScope";
    fab.style.cssText = `
      position: fixed; right: 16px; bottom: 16px; z-index: 9999;
      padding: 10px 16px; font-size: 13px; font-weight: 600;
      background: #1f6feb; color: #fff;
      border: 0; border-radius: 999px; cursor: pointer;
      box-shadow: 0 4px 20px rgba(0,0,0,0.3);
    `;
    fab.addEventListener("click", () => {
      chrome.runtime.sendMessage({ kind: "toggleSidePanel" }).catch(() => {});
    });
    document.body.appendChild(fab);
  }

  // ---- Bootstrapping --------------------------------------------------------
  // We re-scan every few seconds because HF hydrates content lazily and the
  // DOM mutates as the user scrolls. MutationObserver would be more efficient,
  // but a periodic scan is dead-simple and runs at <1ms per pass.
  function tick() {
    injectCatalogButtons();
    injectDetailFab();
  }

  tick();
  setInterval(tick, 2000);
})();
