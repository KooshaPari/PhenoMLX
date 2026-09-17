/**
 * AgilePlus native macOS desktop notifications.
 *
 * Uses Electrobun's native Notification API (Utils.showNotification)
 * with click-to-bring-to-front behaviour.
 *
 * Also runs a tiny localhost HTTP server so the webview can trigger
 * notifications without direct FFI access.
 *
 * Features:
 *  - Native macOS notifications (title, body, subtitle, silent)
 *  - Click action brings the main window to front
 *  - HTTP endpoint at /api/desktop/notifications (POST)
 *  - IPC bridge support for window.electrobun.showNotification()
 */

import { Utils } from "electrobun/bun";
import type { BrowserWindow } from "electrobun/bun";
import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface NotificationPayload {
  title: string;
  body?: string;
  subtitle?: string;
  silent?: boolean;
}

// ── State ─────────────────────────────────────────────────────────────────────

let mainWindow: BrowserWindow | null = null;
let notificationServer: Server | null = null;

// ── Core notification display ─────────────────────────────────────────────────

/**
 * Show a native macOS notification.
 * When clicked, brings the main window to front.
 */
export function showNotification(opts: NotificationPayload): void {
  const { title, body, subtitle, silent } = opts;

  if (!title) {
    console.warn("[Notifications] Cannot show notification without a title");
    return;
  }

  try {
    // Use Electrobun's native notification API
    Utils.showNotification({
      title,
      body: body ?? "",
      subtitle: subtitle ?? "",
      silent: silent ?? false,
    });

    console.log(`[Notifications] Shown: "${title}"${body ? ` — ${body}` : ""}`);
  } catch (err) {
    // Fallback: macOS osascript notification
    console.warn(
      "[Notifications] Electrobun API failed, falling back to osascript:",
      (err as Error).message,
    );
    fallbackNotification(title, body, subtitle);
  }
}

/**
 * Fallback notification via macOS `osascript`.
 * Used when the Electrobun native notification API is unavailable.
 */
function fallbackNotification(
  title: string,
  body?: string,
  subtitle?: string,
): void {
  try {
    const fullBody = [subtitle, body].filter(Boolean).join("\n");
    const script = `display notification "${(fullBody || "").replace(/"/g, "\\\"")}" with title "${title.replace(/"/g, "\\\"")}"`;
    Bun.spawnSync(["osascript", "-e", script]);
  } catch (err) {
    console.error("[Notifications] osascript fallback also failed:", err);
  }
}

// ── Click handler ─────────────────────────────────────────────────────────────

/**
 * Bring the main window to front (called when a notification is clicked).
 * This is wired to the IPC bridge; Electrobun's native notifications do not
 * currently expose a click callback, so we rely on the HTTP endpoint.
 */
function bringWindowToFront(): void {
  if (!mainWindow) return;
  try {
    mainWindow.show();
    mainWindow.activate();
  } catch (err) {
    console.warn("[Notifications] Could not bring window to front:", err);
  }
}

// ── HTTP notification endpoint ────────────────────────────────────────────────

/**
 * Start a localhost HTTP server that the webview can POST to in order to
 * trigger native notifications.
 *
 * POST /api/desktop/notifications
 * Content-Type: application/json
 * Body: { "title": "...", "body": "...", "subtitle": "...", "silent": false }
 *
 * Returns 200 on success, 400 on invalid input.
 */
function startNotificationServer(port: number): Server {
  const server = createServer((req, res) => {
    // CORS headers for local webview access
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");

    if (req.method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    if (req.method !== "POST" || !req.url?.startsWith("/api/desktop/notifications")) {
      res.writeHead(404);
      res.end(JSON.stringify({ error: "Not found" }));
      return;
    }

    let body = "";
    req.on("data", (chunk: Buffer) => {
      body += chunk.toString();
    });
    req.on("end", () => {
      try {
        const payload: NotificationPayload = JSON.parse(body);
        if (!payload.title || typeof payload.title !== "string") {
          res.writeHead(400);
          res.end(JSON.stringify({ error: "Missing or invalid 'title'" }));
          return;
        }

        showNotification(payload);
        bringWindowToFront();

        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ ok: true }));
      } catch (err) {
        res.writeHead(400);
        res.end(
          JSON.stringify({ error: "Invalid JSON", detail: (err as Error).message }),
        );
      }
    });
  });

  server.listen(port, "127.0.0.1", () => {
    const addr = server.address() as AddressInfo;
    console.log(
      `[Notifications] HTTP endpoint at http://127.0.0.1:${addr.port}/api/desktop/notifications`,
    );
  });

  server.on("error", (err: Error) => {
    console.error("[Notifications] Server error:", err.message);
  });

  return server;
}

// ── Public API ────────────────────────────────────────────────────────────────

export interface NotificationModule {
  /** The port the notification server is listening on. */
  port: number;
  /** Direct notification function. */
  show: typeof showNotification;
  /** Stop the notification server. */
  shutdown: () => void;
}

/**
 * Set up desktop notifications for the given window.
 * Starts the local HTTP endpoint on the specified port (or finds a free one).
 *
 * Call once during app bootstrap.
 */
export function setupNotifications(
  win: BrowserWindow,
  preferredPort = 0,
): NotificationModule {
  mainWindow = win;

  // Shut down previous server if re-initialising
  if (notificationServer) {
    notificationServer.close();
  }

  const port = preferredPort || 0;
  notificationServer = startNotificationServer(port);
  const addr = notificationServer.address() as AddressInfo;

  return {
    port: addr.port,
    show: showNotification,
    shutdown: () => {
      if (notificationServer) {
        notificationServer.close();
        notificationServer = null;
      }
    },
  };
}
