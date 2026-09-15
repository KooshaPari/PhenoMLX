// background.ts — service worker.
// Responsibilities:
//   1. Open the side panel on Alt+H (or via toolbar icon on huggingface.co).
//   2. Pass the active tab URL & host into the side panel via URL query params.

const HFSCOPE_DEFAULT = "http://localhost:9190";
const HF_HOSTS = ["huggingface.co", "www.huggingface.co"];

async function getHfscopeHost() {
  return new Promise((res) =>
    chrome.storage.local.get(["hfscopeHost"], (s) =>
      res(s.hfscopeHost || HFSCOPE_DEFAULT)
    )
  );
}

async function isOnHub(tab) {
  if (!tab || !tab.url) return false;
  try {
    const u = new URL(tab.url);
    return HF_HOSTS.includes(u.hostname);
  } catch { return false; }
}

async function toggleSidePanel(tab) {
  if (!tab) return;
  if (!(await isOnHub(tab))) return;

  // We can only open side panel on a user gesture, so when this fires from
  // a keyboard command it's already a user gesture.
  const host = await getHfscopeHost();
  const sidePanelURL =
    chrome.runtime.getURL("side-panel.html") +
    "?" + new URLSearchParams({ hfscope: host, tab: tab.url || "" }).toString();
  await chrome.sidePanel.setOptions({ path: sidePanelURL, enabled: true });
  await chrome.sidePanel.open({ tabId: tab.id });
}

chrome.action.onClicked.addListener((tab) => toggleSidePanel(tab));

chrome.commands.onCommand.addListener(async (command, tab) => {
  if (command === "open-side-panel") await toggleSidePanel(tab);
  if (command === "open-popup" && tab) {
    // No-op — chrome.action opens the popup when clicked.
    // We re-open via a fresh tab if the user wants the popup route for some reason.
    chrome.tabs.create({ url: chrome.runtime.getURL("popup.html?tab=" + tab.id) });
  }
});

// Allow the side panel to query "which tab am I" via long-lived connection.
// Not strictly required today — tab URL is passed via the panel's URL.

export {};
