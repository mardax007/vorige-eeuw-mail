import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The real app path: as if inside Tauri on a phone, with the plugin's own rejections.
vi.mock("./client", () => ({ hasTauri: () => true }));
const scan = vi.fn();
const cancel = vi.fn();
vi.mock("@tauri-apps/plugin-barcode-scanner", () => ({
  scan: (...args: unknown[]) => scan(...args),
  cancel: () => cancel(),
  Format: { QRCode: "QR_CODE" },
}));

import { LATE_STOP_MS, scanCode, stopScan } from "./scanner";

beforeEach(() => {
  scan.mockReset();
  cancel.mockReset().mockResolvedValue(undefined);
});
afterEach(() => vi.useRealTimers());

describe("stopScan", () => {
  it("stops again after the camera's slow first start, so the camera does not stay on", async () => {
    vi.useFakeTimers();
    await stopScan();
    expect(cancel).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(Math.max(...LATE_STOP_MS));
    expect(cancel).toHaveBeenCalledTimes(1 + LATE_STOP_MS.length);
  });

  it("leaves a scan alone that started after the Stoppen", async () => {
    vi.useFakeTimers();
    await stopScan();
    scan.mockReturnValueOnce(new Promise(() => undefined));
    void scanCode();
    await vi.advanceTimersByTimeAsync(Math.max(...LATE_STOP_MS));
    expect(cancel).toHaveBeenCalledTimes(1);
  });
});

describe("scanCode", () => {
  it("returns what the camera read", async () => {
    scan.mockResolvedValueOnce({ content: '{"v":1}' });
    await expect(scanCode()).resolves.toBe('{"v":1}');
    expect(scan).toHaveBeenCalledWith({ windowed: true, formats: ["QR_CODE"] });
  });

  it("returns null after Stoppen, which the plugin rejects as { message: 'cancelled' }", async () => {
    scan.mockRejectedValueOnce({ message: "cancelled" });
    await expect(scanCode()).resolves.toBeNull();
  });

  it("turns another failure into an error with the plugin's text", async () => {
    scan.mockRejectedValueOnce({ message: "camera in use" });
    await expect(scanCode()).rejects.toMatchObject({ code: "VEM-APP-CMD-01", detail: "camera in use" });
  });
});
