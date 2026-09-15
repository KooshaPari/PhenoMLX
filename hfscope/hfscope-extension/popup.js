// popup.js
// Tiny script for the popup — focuses the search box, opens the right URL on submit,
// and stores a sensible default host so the popup never asks twice.

const params = new URLSearchParams(location.search);
const tabId = params.get("tab") || null;

const HFSCOPE_DEFAULT = "http://localhost:9190";

const $ = (sel) => document.querySelector(sel);
const host = $("#host");
const q = $("#q");
const kind = $("#kind");
const limit = $("#limit");
const btn = $("#open");
const tab = $("#open-tab");

// Restore last-used host + last query
chrome.storage.local.get(["hfscopeHost", "lastQ"], (s) => {
  const hostVal = s.hfscopeHost || HFSCOPE_DEFAULT;
  if (host) host.textContent = hostVal;
  if (s.lastQ) q.value = s.lastQ;
});

// Pull the active tab URL so the open-tab button can deep-link
if (tabId) {
  chrome.tabs.get(tabId, (t) => {
    if (t && t.url && t.url.includes("huggingface.co")) {
      tab.style.display = "inline";
      tab.href = t.url;
    }
  });
}

function submit() {
  const hostVal = (host && host.textContent) || HFSCOPE_DEFAULT;
  const url = `${hostVal}/results?` + new URLSearchParams({
    q: q.value || "",
    type: kind.value,
    limit: limit.value || "20",
  }).toString();
  chrome.storage.local.set({ hfscopeHost: hostVal, lastQ: q.value || "" });
  chrome.tabs.create({ url });
  window.close();
}

btn.addEventListener("click", submit);
q.addEventListener("keydown", (e) => { if (e.key === "Enter") submit(); });
