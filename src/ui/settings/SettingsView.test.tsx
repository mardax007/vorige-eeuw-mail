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
