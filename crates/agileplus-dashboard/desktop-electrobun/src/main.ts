/**
 * AgilePlus Desktop Shell — Electrobun main process
 *
 * When launched from agileplus-launch.ps1:
 * - The daemon (agileplus-dashboard) is already running via the launcher
 * - RENDERER_URL is set to http://localhost:$AGILEPLUS_DASHBOARD_PORT
 * - This window simply displays the running dashboard
 *
 * Features:
 *  - Loads renderer from RENDERER_URL env (set by launcher)
 *  - Fallback to bundled views://app/index.html if unreachable
 *  - Standard window with hiddenInset title bar (macOS), 1400x900
 *  - Minimal app menu
 */
import { BrowserWindow, ApplicationMenu } from "electrobun/bun";
import { join } from "node:path";

// ── Config ────────────────────────────────────────────────────────────────────
const APP_NAME = process.env.APP_NAME ?? "AgilePlus";

// Fallback page (shown while waiting for renderer to be reachable)
const RENDERER_FALLBACK = "views://app/index.html";

// Live renderer URL (set by launcher via RENDERER_URL=http://localhost:$PORT)
const RENDERER_URL = process.env.RENDERER_URL ?? "http://localhost:8770";

// ── Window ────────────────────────────────────────────────────────────────────
function createMainWindow(): BrowserWindow {
  const win = new BrowserWindow({
    title: APP_NAME,
    url: RENDERER_FALLBACK,
    frame: {
      x: 0,
      y: 0,
      width: parseInt(process.env.WINDOW_WIDTH ?? "1400"),
      height: parseInt(process.env.WINDOW_HEIGHT ?? "900"),
    },
    titleBarStyle: "hiddenInset",
  });

  // Tell the fallback page which renderer URL to poll and navigate to
  try {
    win.webview.executeJavascript(`window.__RENDERER_URL__ = ${JSON.stringify(RENDERER_URL)};`);
  } catch {
    // Webview not ready yet; fallback page will use its default
  }

  return win;
}

// ── Menu ──────────────────────────────────────────────────────────────────────
function setupMenu(win: BrowserWindow): void {
  ApplicationMenu.setApplicationMenu([
    {
      label: APP_NAME,
      submenu: [
        { role: "about" },
        { type: "separator" },
        { role: "services" },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { role: "quit" },
      ],
    },
    {
      label: "Edit",
      submenu: [
        { role: "undo" },
        { role: "redo" },
        { type: "separator" },
        { role: "cut" },
        { role: "copy" },
        { role: "paste" },
        { role: "selectAll" },
      ],
    },
    {
      label: "View",
      submenu: [
        { role: "reload" },
        { role: "forceReload" },
        { role: "toggleDevTools" },
        { type: "separator" },
        { role: "togglefullscreen" },
      ],
    },
  ]);
}

// ── Bootstrap ─────────────────────────────────────────────────────────────────
async function main(): Promise<void> {
  const win = createMainWindow();
  setupMenu(win);
  console.log(`[${APP_NAME}] Desktop shell launched → ${RENDERER_URL} (fallback: ${RENDERER_FALLBACK})`);
}

main().catch((err) => {
  console.error(`[${APP_NAME}] Fatal:`, err);
  process.exit(1);
});
