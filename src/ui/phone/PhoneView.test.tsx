import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { resetMockBackend } from "../../api/mock";
import * as scanner from "../../api/scanner";

vi.mock("../../api/scanner", async (original) => {
  const real = await original<typeof import("../../api/scanner")>();
  return {
    ...real,
    askCamera: vi.fn(real.askCamera),
    scanCode: vi.fn(real.scanCode),
    stopScan: vi.fn(real.stopScan),
    openAppSettings: vi.fn(real.openAppSettings),
  };
});

/** "Telefoon koppelen" sits under the folder pane's "more" button. */
async function openPhonePage(user: ReturnType<typeof userEvent.setup>, nav: HTMLElement) {
  await user.click(within(nav).getByRole("button", { expanded: false }));
  await user.click(within(nav).getByRole("button", { name: "Telefoon koppelen" }));
}

async function openOnPhone() {
  const real = api.appInfo;
  vi.spyOn(api, "appInfo").mockImplementation(async () => ({ ...(await real()), isPhone: true }));
  const user = userEvent.setup();
  render(<App />);
  const nav = await screen.findByRole("navigation", { name: "Mappen" });
  await openPhonePage(user, nav);
  await screen.findByRole("heading", { name: "Telefoon koppelen" });
  return user;
}

// One-shot overrides only: the scanner mocks keep the real (browser) behaviour between tests.
beforeEach(() => {
  resetMockBackend();
  vi.clearAllMocks();
});
afterEach(() => vi.restoreAllMocks());

describe("Telefoon koppelen on the phone", () => {
  it("scans the PC's code and says the phone is linked", async () => {
    const user = await openOnPhone();
    expect(screen.queryByRole("button", { name: "Code maken" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Code scannen" }));
    expect(await screen.findByText("De telefoon is gekoppeld. Uw contactpersonen komen er zo op.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Koppeling verbreken" })).toBeTruthy();
    expect(await api.phonePaired()).toBe(true);
  });

  it("shows only the hint and a big Stoppen while the camera runs, and Stoppen goes back", async () => {
    let finish: (text: string | null) => void = () => undefined;
    vi.mocked(scanner.scanCode).mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
    vi.mocked(scanner.stopScan).mockImplementationOnce(async () => finish(null));
    const user = await openOnPhone();
    await user.click(screen.getByRole("button", { name: "Code scannen" }));
    const overlay = await screen.findByRole("dialog", { name: "Code scannen" });
    expect(within(overlay).getByText("Richt de camera op de code op het scherm van de computer.")).toBeTruthy();
    expect(document.documentElement.classList.contains("scanning")).toBe(true);
    await user.click(within(overlay).getByRole("button", { name: "Stoppen" }));
    await screen.findByRole("button", { name: "Code scannen" });
    expect(document.documentElement.classList.contains("scanning")).toBe(false);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(await api.phonePaired()).toBe(false);
  });

  it("says how to allow the camera after a no", async () => {
    vi.mocked(scanner.askCamera).mockResolvedValueOnce(false);
    const user = await openOnPhone();
    await user.click(screen.getByRole("button", { name: "Code scannen" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("zet Camera aan");
    await user.click(within(alert).getByRole("button", { name: "Instellingen openen" }));
    expect(scanner.openAppSettings).toHaveBeenCalled();
    expect(scanner.scanCode).not.toHaveBeenCalled();
  });

  it("shows the error for a code that is not ours", async () => {
    vi.mocked(scanner.scanCode).mockResolvedValueOnce("https://example.test/");
    const user = await openOnPhone();
    await user.click(screen.getByRole("button", { name: "Code scannen" }));
    expect(await screen.findByText("Deze code is geen koppelcode van Vorige Eeuw Mail.")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Code scannen" })).toBeTruthy());
    expect(await api.phonePaired()).toBe(false);
  });

  it("keeps the PC's Code maken on a PC", async () => {
    const user = userEvent.setup();
    render(<App />);
    const nav = await screen.findByRole("navigation", { name: "Mappen" });
    await openPhonePage(user, nav);
    expect(await screen.findByRole("button", { name: "Code maken" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Code scannen" })).toBeNull();
  });
});
