/**
 * AgilePlus system tray — macOS menu bar icon with context menu and badge support.
 *
 * Uses Electrobun's built-in Tray API with a programmatically generated
 * icon (small colored PNG) so the app works out of the box on macOS
 * without requiring a separate icon asset.
 *
 * Features:
 *  - macOS menu bar icon with AgilePlus brand colour
 *  - Context menu: Show/Hide, New Epic, New Story, Notification Prefs, Quit
 *  - Quick-action items dispatch JavaScript calls to the webview
 *  - Unread count badge shown as a tooltip / title
 *  - Tray icon configurable via `TRAY_ICON_PATH` env var
 */

import { Tray, Utils } from "electrobun/bun";
import type { BrowserWindow } from "electrobun/bun";
import type { MenuItemConfig } from "electrobun/bun";
import { join } from "node:path";
import { writeFileSync, existsSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { deflateSync } from "node:zlib";

// ── Constants ────────────────────────────────────────────────────────────────

const APP_NAME = process.env.APP_NAME ?? "AgilePlus";
const TRAY_ICON_SIZE = 22;
// Indigo brand colour (AgilePlus palette)
const TRAY_R = 99;
const TRAY_G = 102;
const TRAY_B = 241;

// ── State ─────────────────────────────────────────────────────────────────────

let tray: Tray | null = null;
let unreadCount = 0;
let isWindowVisible = true;

// ── CRC-32 (for embedded PNG) ─────────────────────────────────────────────────

function crc32(buf: Buffer): number {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc ^= buf[i]!;
    for (let j = 0; j < 8; j++) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type: string, data: Buffer): Buffer {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeB = Buffer.from(type, "ascii");
  const crcInput = Buffer.concat([typeB, data]);
  const crcV = crc32(crcInput);
  const crcB = Buffer.alloc(4);
  crcB.writeUInt32BE(crcV, 0);
  return Buffer.concat([len, typeB, data, crcB]);
}

/**
 * Generate a minimal solid-colour PNG for the tray icon.
 * Returns the path to the cached file.
 */
function generateTrayIcon(): string {
  const cachedPath = join(tmpdir(), "agileplus-tray-icon.png");

  // Skip generation if already cached
  if (existsSync(cachedPath)) return cachedPath;

  const w = TRAY_ICON_SIZE;
  const h = TRAY_ICON_SIZE;

  // Raw pixel data: filter byte + RGBA per row
  const stride = 1 + w * 4;
  const raw = Buffer.alloc(h * stride);
  for (let y = 0; y < h; y++) {
    const rowOff = y * stride;
    raw[rowOff] = 0; // filter: none
    for (let x = 0; x < w; x++) {
      const px = rowOff + 1 + x * 4;
      raw[px] = TRAY_R;
      raw[px + 1] = TRAY_G;
      raw[px + 2] = TRAY_B;
      raw[px + 3] = 255;
    }
  }

  const compressed = deflateSync(raw);

  // Assemble PNG
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;   // bit depth
  ihdr[9] = 6;   // colour type RGBA
  ihdr[10] = 0;  // compression
  ihdr[11] = 0;  // filter
  ihdr[12] = 0;  // interlace

  const png = Buffer.concat([
    signature,
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", compressed),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);

  writeFileSync(cachedPath, png);
  return cachedPath;
}

// ── Dispatch helper ───────────────────────────────────────────────────────────

/**
 * Execute a JavaScript call on the main window's webview.
 * Silently no-ops if the window or webview are unavailable.
 */
function dispatchJs(win: BrowserWindow, code: string): void {
  try {
    win.webview.executeJavascript(code);
  } catch (err) {
    console.warn(`[Tray] dispatchJs failed:`, (err as Error).message);
  }
}

// ── Context menu builder ──────────────────────────────────────────────────────

function buildTrayMenu(win: BrowserWindow): MenuItemConfig[] {
  return [
    {
      type: "normal",
      label: isWindowVisible ? "Hide Window" : "Show Window",
      action: "toggle-window",
      enabled: true,
    },
    { type: "separator" },
    {
      type: "normal",
      label: "New Epic",
      action: "new-epic",
      enabled: true,
    },
    {
      type: "normal",
      label: "New Story",
      action: "new-story",
      enabled: true,
    },
    { type: "separator" },
    {
      type: "normal",
      label: "Notification Preferences…",
      action: "notification-prefs",
      enabled: true,
    },
    { type: "separator" },
    { type: "normal", label: "Quit", action: "quit", enabled: true },
  ];
}

// ── Menu click handler ────────────────────────────────────────────────────────

function handleTrayAction(win: BrowserWindow, action: string): void {
  switch (action) {
    case "toggle-window":
      toggleWindowVisibility(win);
      break;
    case "new-epic":
      dispatchJs(win, "window.__agileplus?.createEpic?.()");
      break;
    case "new-story":
      dispatchJs(win, "window.__agileplus?.createStory?.()");
      break;
    case "notification-prefs":
      dispatchJs(win, "window.__agileplus?.openNotificationPrefs?.()");
      break;
    case "quit":
      Utils.quit();
      break;
    default:
      console.warn(`[Tray] Unknown action: ${action}`);
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * Create and configure the macOS menu-bar tray icon.
 * Call once during app bootstrap with the main `BrowserWindow`.
 */
export function setupTray(win: BrowserWindow): Tray {
  if (tray) {
    tray.remove();
  }

  const iconPath = process.env.TRAY_ICON_PATH ?? generateTrayIcon();

  tray = new Tray({
    title: "",
    image: iconPath,
    template: true,
    width: TRAY_ICON_SIZE,
    height: TRAY_ICON_SIZE,
  });

  // Set initial tooltip
  tray.setTitle(badgeLabel());

  // Build and set context menu
  const menu = buildTrayMenu(win);
  tray.setMenu(menu);

  // Listen for tray menu item clicks
  tray.on("tray-clicked", (event: unknown) => {
    const ev = event as { data?: { action?: string } };
    const action = ev?.data?.action;
    if (action) {
      handleTrayAction(win, action);
      // Rebuild menu so Show/Hide label reflects current state
      tray?.setMenu(buildTrayMenu(win));
    }
  });

  // Also handle application menu quit for consistency
  tray.on("tray-clicked", () => {
    /* tray icon double-click / click – could bring window to front */
  });

  console.log(`[Tray] System tray active (icon: ${iconPath})`);
  return tray;
}

/**
 * Update the unread-count badge.
 * macOS does not natively badge tray icons, so we set the tray tooltip/title.
 */
export function updateBadge(count: number): void {
  unreadCount = Math.max(0, count);
  if (tray) {
    tray.setTitle(badgeLabel());
  }
}

/**
 * Get the current unread count.
 */
export function getUnreadCount(): number {
  return unreadCount;
}

/**
 * Toggle the main window between hidden and visible.
 */
export function toggleWindowVisibility(win: BrowserWindow): void {
  if (isWindowVisible) {
    win.hide();
    isWindowVisible = false;
  } else {
    win.show();
    win.activate();
    isWindowVisible = true;
  }
}

/**
 * Check whether the main window is currently visible.
 */
export function getIsWindowVisible(): boolean {
  return isWindowVisible;
}

/**
 * Synchronise the window-visibility tracking state (call on window focus/blur).
 */
export function setWindowVisible(visible: boolean): void {
  isWindowVisible = visible;
}

// ── Internals ─────────────────────────────────────────────────────────────────

function badgeLabel(): string {
  return unreadCount > 0
    ? `${APP_NAME} (${unreadCount} unread)`
    : APP_NAME;
}
