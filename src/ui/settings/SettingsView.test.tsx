import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { resetMockBackend } from "../../api/mock";

async function openSettings(isPhone: boolean) {
  const real = api.appInfo;
  vi.spyOn(api, "appInfo").mockImplementation(async () => ({ ...(await real()), isPhone }));
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  await user.click(screen.getByRole("button", { name: "Instellingen" }));
  await screen.findByRole("heading", { name: "Instellingen" });
  return user;
}

beforeEach(() => resetMockBackend());
afterEach(() => vi.restoreAllMocks());

describe("Lettergrootte", () => {
  it("makes all text larger and smaller within its limits, and keeps the choice", async () => {
    const save = vi.spyOn(api, "saveSettings");
    const user = await openSettings(false);
    const smaller = screen.getByRole("button", { name: /Kleiner/ }) as HTMLButtonElement;
    const larger = screen.getByRole("button", { name: /Groter/ }) as HTMLButtonElement;
    expect(smaller.disabled).toBe(true);
    expect(screen.getByText("Nu: 18 (gewoon)")).toBeTruthy();
    await user.click(larger);
    expect(await screen.findByText("Nu: 20")).toBeTruthy();
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: 20 }));
    expect(document.documentElement.style.getPropertyValue("--text-size")).toBe("20px");
    for (let i = 0; i < 10; i++) if (!larger.disabled) await user.click(larger);
    expect(await screen.findByText("Nu: 32")).toBeTruthy();
    expect(larger.disabled).toBe(true);
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: 32 }));
    expect((await api.getSettings()).textSize).toBe(32);
    await user.click(smaller);
    expect(await screen.findByText("Nu: 30")).toBeTruthy();
  });
});

describe("Nieuwe mail direct melden (decision 24)", () => {
  it("is offered on a phone, off at first, and saved when switched on", async () => {
    const save = vi.spyOn(api, "saveSettings");
    const user = await openSettings(true);
    const box = screen.getByRole("checkbox", { name: "Nieuwe mail direct melden" });
    expect((box as HTMLInputElement).checked).toBe(false);
    expect(box.getAttribute("aria-describedby")).toBe("set-background-hint");
    expect(screen.queryByRole("checkbox", { name: "Starten met Windows" })).toBeNull();
    await user.click(box);
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ backgroundMail: true }));
  });

  it("is not on the PC, which watches from next to the clock", async () => {
    await openSettings(false);
    expect(screen.queryByRole("checkbox", { name: "Nieuwe mail direct melden" })).toBeNull();
    expect(screen.getByRole("checkbox", { name: "Starten met Windows" })).toBeTruthy();
  });
});

describe("Reservekopie (C14)", () => {
  it("makes a backup with progress and says where it went", async () => {
    const user = await openSettings(false);
    await user.click(screen.getByRole("button", { name: "Reservekopie maken" }));
    expect(await screen.findByText(/De reservekopie wordt gemaakt/)).toBeTruthy();
    expect(await screen.findByText(/Klaar\. De reservekopie staat in de map E:\\Vorige Eeuw Mail reservekopie/, {}, { timeout: 4000 })).toBeTruthy();
  });

  it("stops when asked and says nothing was left behind", async () => {
    const user = await openSettings(false);
    await user.click(screen.getByRole("button", { name: "Reservekopie maken" }));
    await user.click(await screen.findByRole("button", { name: "Stoppen" }));
    expect(await screen.findByText(/Gestopt\. Er is niets half op de USB-stick/)).toBeTruthy();
  });

  it("asks once before putting a backup back, naming what is in it", async () => {
    const restore = vi.spyOn(api, "startRestore");
    const user = await openSettings(false);
    await user.click(screen.getByRole("button", { name: "Reservekopie terugzetten" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toMatch(/3210 berichten van ria\.jansen@solcon\.nl, ria\.jansen1950@gmail\.com/);
    await user.click(screen.getByRole("button", { name: "Annuleren" }));
    expect(restore).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Reservekopie terugzetten" }));
    await user.click(await screen.findByRole("button", { name: "Terugzetten" }));
    expect(restore).toHaveBeenCalledWith("E:\\Vorige Eeuw Mail reservekopie 2026-11-01");
    expect(await screen.findByText("Klaar. Het programma start zo opnieuw, met de reservekopie.", {}, { timeout: 4000 })).toBeTruthy();
  });

  it("is not offered on a phone", async () => {
    await openSettings(true);
    expect(screen.queryByRole("button", { name: "Reservekopie maken" })).toBeNull();
  });
});
