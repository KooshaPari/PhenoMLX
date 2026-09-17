/**
 * AgilePlus IPC bridge — bi-directional communication between the
 * webview (React/Vite app) and the native Electrobun main process.
 *
 * The webview can trigger native notifications by calling
 * `window.electrobun.showNotification({ title, body })`.
 *
 * The bridge also exposes a preload script that gets injected into the
 * webview to set up the `window.electrobun` object.
 *
 * Architecture:
 *  - We inject a preload script into BrowserView that exposes
 *    `window.electrobun` methods
 *  - The webview's IPC calls to showNotification are relayed to
 *    the notification module via a callback
 *  - For the webview, we use RPC over the internal Bun bridge
 *    (Electrobun's event system) or fall back to fetch-based
 *    communication via the localhost HTTP endpoint
 */

import type { BrowserWindow } from "electrobun/bun";
import type { NotificationPayload } from "./notifications";
import { showNotification } from "./notifications";
import { updateBadge, setWindowVisible, getIsWindowVisible } from "./tray";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface IpcBridge {
  /** Inject the preload script into the webview. */
  inject: () => void;
  /** Teardown any bridges. */
  destroy: () => void;
}

export interface ElectrobunWebviewApi {
  showNotification: (opts: NotificationPayload) => void;
  setBadge: (count: number) => void;
  getPlatform: () => string;
  isWindowVisible: () => boolean;
}

// ── Preload script ────────────────────────────────────────────────────────────

/**
 * JavaScript string that gets injected into the webview context.
 * It exposes `window.electrobun` with notification + badge methods.
 *
 * Communication strategy:
 *  - Primary: `window.__electrobunBunBridge.postMessage()`
 *    (Electrobun's built-in RPC bridge to the main process)
 *  - Fallback: HTTP POST to the local notification server
 *
 * The notification server port is injected at bootstrap time.
 */
function buildPreloadScript(notifPort: number): string {
  const safePort = JSON.stringify(notifPort);
  return `
(function() {
  if (window.__agileplusElectrobunInjected) return;
  window.__agileplusElectrobunInjected = true;

  var NOTIF_PORT = ${safePort};

  window.electrobun = {
    showNotification: function(opts) {
      if (!opts || !opts.title) return;

      // Primary: use ElectroBun's bunBridge
      try {
        var bridge = window.__electrobunBunBridge;
        if (bridge && typeof bridge.postMessage === 'function') {
          bridge.postMessage(JSON.stringify({
            type: 'show-notification',
            title: opts.title,
            body: opts.body || '',
            subtitle: opts.subtitle || '',
            silent: !!opts.silent
          }));
          return;
        }
      } catch (e) {
        // fall through
      }

      // Fallback: HTTP POST to local notification endpoint
      try {
        var xhr = new XMLHttpRequest();
        xhr.open('POST', 'http://127.0.0.1:' + NOTIF_PORT + '/api/desktop/notifications', true);
        xhr.setRequestHeader('Content-Type', 'application/json');
        xhr.send(JSON.stringify({
          title: opts.title,
          body: opts.body || '',
          subtitle: opts.subtitle || '',
          silent: !!opts.silent
        }));
      } catch (e) {
        console.warn('[electrobun] showNotification failed:', e);
      }
    },

    setBadge: function(count) {
      try {
        var bridge = window.__electrobunBunBridge;
        if (bridge && typeof bridge.postMessage === 'function') {
          bridge.postMessage(JSON.stringify({
            type: 'set-badge',
            count: Math.max(0, parseInt(count, 10) || 0)
          }));
        }
      } catch (e) {
        // silently ignore
      }
    },

    getPlatform: function() {
      return 'darwin';
    },

    isWindowVisible: function() {
      return true;
    }
  };
})();
`;
}

// ── IPC message handler ───────────────────────────────────────────────────────

/**
 * Handle incoming messages from the webview's bunBridge.
 * Dispatches to the appropriate native module.
 */
function handleWebviewMessage(message: string): void {
  try {
    const parsed = JSON.parse(message) as {
      type: string;
      [key: string]: unknown;
    };

    switch (parsed.type) {
      case "show-notification": {
        showNotification({
          title: parsed.title as string,
          body: parsed.body as string | undefined,
          subtitle: parsed.subtitle as string | undefined,
          silent: parsed.silent as boolean | undefined,
        });
        break;
      }
      case "set-badge": {
        const count = parseInt(String(parsed.count ?? "0"), 10);
        updateBadge(count);
        break;
      }
      default:
        // Unknown message types are silently ignored
        break;
    }
  } catch {
    // Malformed messages are silently ignored
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * Set up the IPC bridge between the webview and the native main process.
 *
 * Injects a preload script into the window's webview that exposes
 * `window.electrobun.showNotification()`, `window.electrobun.setBadge()`,
 * etc., and wires the main-process-side message handler.
 *
 * @param win - The main BrowserWindow
 * @param notificationPort - The port the notification HTTP server is listening on
 */
export function setupIpc(win: BrowserWindow, notificationPort: number): IpcBridge {
  const preloadScript = buildPreloadScript(notificationPort);

  // Inject the preload into the webview
  try {
    win.webview.executeJavascript(preloadScript);
    console.log(`[IPC] Preload script injected (notif port ${notificationPort})`);
  } catch (err) {
    console.warn("[IPC] Preload injection failed (webview may not be ready yet):", err);
    // Retry after a short delay
    setTimeout(() => {
      try {
        win.webview.executeJavascript(preloadScript);
        console.log(`[IPC] Preload script injected on retry`);
      } catch (retryErr) {
        console.warn("[IPC] Retry also failed:", retryErr);
      }
    }, 2000);
  }

  // Wire window visibility tracking
  try {
    win.webview.executeJavascript(`
      (function() {
        document.addEventListener('visibilitychange', function() {
          if (document.visibilityState === 'visible') {
            window.__agileplusWindowVisible = true;
          } else {
            window.__agileplusWindowVisible = false;
          }
        });
      })();
    `);
  } catch {
    // non-critical
  }

  console.log(`[IPC] Bridge established`);

  return {
    inject: () => {
      try {
        win.webview.executeJavascript(preloadScript);
      } catch {
        // silent
      }
    },
    destroy: () => {
      // Cleanup is minimal — the preload script uses a guard variable to
      // prevent double-injection; removing the bridge is handled by
      // window.close / app lifecycle
    },
  };
}
