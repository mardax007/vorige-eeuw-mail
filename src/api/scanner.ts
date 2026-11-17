// The phone's camera for "Telefoon koppelen" (phase 5 task 9), through Tauri's barcode-scanner
// plugin. Only the phone calls this. In a plain browser (tests, `npm run dev`) it pretends the
// camera read the code at once.

import { hasTauri } from "./client";
import { errorText, toVemError } from "./errors";

/** What the browser pretends to scan; the mock backend accepts it as a code from the PC. */
export const MOCK_PAIR_TEXT = '{"v":1,"account":"jan@example.nl","key":"proef"}';

async function plugin() {
  return import("@tauri-apps/plugin-barcode-scanner");
}

/** Asks Android for the camera when needed. True when it may be used. */
export async function askCamera(): Promise<boolean> {
  if (!hasTauri()) return true;
  try {
    const p = await plugin();
    let state = await p.checkPermissions();
    if (state !== "granted") state = await p.requestPermissions();
    return state === "granted";
  } catch (e: unknown) {
    throw toVemError(e);
  }
}

/**
 * Reads one QR code. The camera shows behind the page (the page turns see-through while
 * scanning), so the page can keep its own Stoppen button on top. Null when stopped.
 */
export async function scanCode(): Promise<string | null> {
  if (!hasTauri()) return MOCK_PAIR_TEXT;
  scanCount += 1;
  try {
    const p = await plugin();
    const found = await p.scan({ windowed: true, formats: [p.Format.QRCode] });
    return found.content;
  } catch (e: unknown) {
    // Stoppen makes the plugin reject the scan with { message: "cancelled" }.
    if (errorText(e).toLowerCase().includes("cancel")) return null;
    throw toVemError(e);
  }
}

/** Counts scans, so a late Stoppen never switches off a scan that started after it. */
let scanCount = 0;

/**
 * When to stop the camera once more after Stoppen. The plugin switches the camera on only when
 * Android's camera service has started, which the first time takes a few seconds; a Stoppen
 * before that finds nothing to switch off, and the camera would then stay on (seen on the
 * emulator, 17 November 2026: the camera came on five seconds after Stoppen). A later stop
 * catches it.
 */
export const LATE_STOP_MS = [1500, 5000, 10000];

export async function stopScan(): Promise<void> {
  if (!hasTauri()) return;
  const scanAtStop = scanCount;
  try {
    const p = await plugin();
    await p.cancel();
    for (const ms of LATE_STOP_MS) {
      setTimeout(() => {
        if (scanCount === scanAtStop) void p.cancel().catch(() => undefined);
      }, ms);
    }
  } catch (e: unknown) {
    throw toVemError(e);
  }
}

/** Android's settings page for this app, where the camera can be allowed after a "no". */
export async function openAppSettings(): Promise<void> {
  if (!hasTauri()) return;
  try {
    await (await plugin()).openAppSettings();
  } catch (e: unknown) {
    throw toVemError(e);
  }
}
