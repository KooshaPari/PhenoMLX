// options.js
const host = document.getElementById("host");
const save = document.getElementById("save");
const saved = document.getElementById("saved");

chrome.storage.local.get(["hfscopeHost"], (s) => {
  host.value = s.hfscopeHost || "http://localhost:9190";
});

save.addEventListener("click", () => {
  chrome.storage.local.set({ hfscopeHost: host.value.trim() }, () => {
    saved.style.display = "inline";
    setTimeout(() => (saved.style.display = "none"), 1500);
  });
});
