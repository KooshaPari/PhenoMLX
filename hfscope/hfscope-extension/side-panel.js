// side-panel.js
// Mounted when user opens the side panel (Alt+H or via toolbar icon on huggingface.co).
// Strategy: iframe HFScope's /embed/results, which serves HTML fragments with
// query string driven from the active HF tab URL. Cross-origin via window.postMessage.

const root = document.getElementById("hfscope-root");

/** Set the iframe to a given HFScope embed URL. */
function mount(hfscopeURL, query) {
  const iframe = document.createElement("iframe");
  iframe.src = `${hfscopeURL}/embed/results?q=${encodeURIComponent(query || "")}&type=models&limit=20`;
  iframe.setAttribute("allow", "clipboard-read; clipboard-write");
  iframe.setAttribute("title", "HFScope results");
  root.innerHTML = "";
  root.appendChild(iframe);
}

/** Read query string off a huggingface.co URL — pulls /models?… or similar. */
function parseHFQuery(u) {
  try {
    const url = new URL(u);
    const p = url.pathname.split("/").filter(Boolean); // ["models"], ["meta-llama","Llama-2-7b-hf"]
    const params = url.searchParams;
    if (p[0] === "models" && p.length === 1) {
      return params.get("search") || params.get("q") || "";
    }
    if (p[0] === "models" && p.length >= 3) {
      // on a model page — return that model id for a /embed/model/{id} view
      return `${p.slice(1).join("/")}`;
    }
    return "";
  } catch { return ""; }
}

/** Get the active tab's URL — fires once per panel mount. */
const params = new URLSearchParams(location.search);
const hfscopeHost = params.get("hfscope") || "http://localhost:9190";
const overrideQuery = params.get("q") || "";
const activeTabURL = params.get("tab") || "";

const query = overrideQuery || (activeTabURL ? parseHFQuery(activeTabURL) : "");
mount(hfscopeHost, query);
