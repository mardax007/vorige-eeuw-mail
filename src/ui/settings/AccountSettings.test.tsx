import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { resetMockBackend } from "../../api/mock";
import { createSampleState } from "../../api/mock/data";
import { errorText } from "../../i18n";
import { oauthDonePage, providerOf, signInRoute } from "./signIn";

afterEach(() => vi.restoreAllMocks());

async function openSettings() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  await user.click(screen.getByRole("button", { name: "Instellingen" }));
  await screen.findByRole("heading", { name: "Instellingen" });
  return user;
}

function accountBlock(name: string): HTMLElement {
  const heading = screen.getAllByRole("heading", { level: 4 }).find((h) => h.textContent?.startsWith(`${name} `));
  return heading?.closest(".account-settings") as HTMLElement;
}

describe("signInRoute", () => {
  it("reads the provider from the address", () => {
    expect(providerOf("ria@gmail.com")).toBe("google");
    expect(providerOf("ria@Hotmail.com")).toBe("microsoft");
    expect(providerOf("ria@solcon.nl")).toBe("password");
  });

  it("offers the browser for Microsoft, for Google when available, and after an expired sign-in", () => {
    expect(signInRoute("microsoft", false)).toBe("browser");
    expect(signInRoute("google", true)).toBe("browser");
    expect(signInRoute("google", false)).toBe("password");
    expect(signInRoute("password", false)).toBe("password");
    expect(signInRoute("password", false, { code: "VEM-AUTH-LOGIN-02" })).toBe("browser");
  });
});

describe("new password", () => {
  it("saves a new password that works", async () => {
    const spy = vi.spyOn(api, "updatePassword");
    const user = await openSettings();
    const block = within(accountBlock("Ria Jansen"));
    await user.type(block.getByLabelText("Nieuw wachtwoord voor Ria Jansen"), "nieuw-geheim");
    await user.click(block.getByRole("button", { name: "Wachtwoord opslaan" }));
    expect(await block.findByText("Het nieuwe wachtwoord werkt en is opgeslagen.")).toBeTruthy();
    expect(spy).toHaveBeenCalledWith("acc-solcon", "nieuw-geheim");
  });

  it("shows the refusal per errors.md and keeps the old password", async () => {
    const user = await openSettings();
    const block = within(accountBlock("Ria Jansen"));
    await user.type(block.getByLabelText("Nieuw wachtwoord voor Ria Jansen"), "fout");
    await user.click(block.getByRole("button", { name: "Wachtwoord opslaan" }));
    expect(await block.findByText("Inloggen bij uw postvak is niet gelukt.")).toBeTruthy();
    expect(block.queryByText("Het nieuwe wachtwoord werkt en is opgeslagen.")).toBeNull();
  });

  it("shows the mailbox's current error right above, with a next step that names real buttons", async () => {
    const state = createSampleState();
    const error = { code: "VEM-SMTP-LOGIN-01", serverReply: "535 5.7.8 Error: authentication failed", detail: null };
    resetMockBackend({ ...state, accounts: state.accounts.map((a) => (a.id === "acc-solcon" ? { ...a, lastError: error } : a)) });
    await openSettings();
    const block = within(accountBlock("Ria Jansen"));
    expect(block.getByText("Inloggen om mail te versturen is niet gelukt.")).toBeTruthy();
    expect(errorText("VEM-SMTP-LOGIN-01").next).toContain("Wachtwoord opslaan");
    expect(block.getByRole("button", { name: "Wachtwoord opslaan" })).toBeTruthy();
  });
});

describe("Opnieuw inloggen", () => {
  it("signs a Google mailbox in again through the browser", async () => {
    const spy = vi.spyOn(api, "reauthAccount");
    const user = await openSettings();
    const block = within(accountBlock("Ria Jansen (Gmail)"));
    expect(block.queryByLabelText(/Nieuw wachtwoord/)).toBeNull();
    await user.click(await block.findByRole("button", { name: "Opnieuw inloggen" }));
    expect(block.getByText("Log in via het venster van uw internetprogramma. Kom daarna hier terug.")).toBeTruthy();
    expect(await block.findByText("U bent weer ingelogd.", undefined, { timeout: 3000 })).toBeTruthy();
    expect(spy.mock.calls[0]).toEqual([
      "acc-gmail",
      "U bent ingelogd. U kunt dit venster sluiten en teruggaan naar Vorige Eeuw Mail.",
    ]);
    expect(errorText("VEM-AUTH-LOGIN-02").next).toContain("Opnieuw inloggen");
  });

  it("follows how the core says a mailbox signs in, not the address", async () => {
    const state = createSampleState();
    resetMockBackend({
      ...state,
      accounts: state.accounts.map((a) =>
        a.id === "acc-solcon" ? { ...a, signIn: "google" as const } : a.id === "acc-gmail" ? { ...a, signIn: "password" as const } : a,
      ),
    });
    await openSettings();
    expect(await within(accountBlock("Ria Jansen")).findByRole("button", { name: "Opnieuw inloggen" })).toBeTruthy();
    expect(within(accountBlock("Ria Jansen (Gmail)")).getByLabelText("Nieuw wachtwoord voor Ria Jansen (Gmail)")).toBeTruthy();
  });

  it("goes back without an error after Stoppen", async () => {
    const user = await openSettings();
    const block = within(accountBlock("Ria Jansen (Gmail)"));
    await user.click(await block.findByRole("button", { name: "Opnieuw inloggen" }));
    await user.click(block.getByRole("button", { name: "Stoppen" }));
    expect(await block.findByRole("button", { name: "Opnieuw inloggen" })).toBeTruthy();
    await waitFor(() => expect(block.queryByText(/niet afgemaakt/)).toBeNull());
    expect(block.queryByText("U bent weer ingelogd.")).toBeNull();
  });
});

describe("settings page", () => {
  it("puts Postvak verwijderen on its own row, not filled red, and Afsluiten at the bottom", async () => {
    await openSettings();
    const remove = within(accountBlock("Ria Jansen")).getByRole("button", { name: "Postvak verwijderen" });
    expect(remove.className).toBe("danger-outline");
    expect(remove.closest(".danger-zone")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Afsluiten" })).toBeTruthy();
    expect(document.querySelector(".top-bar")?.textContent).not.toContain("Afsluiten");
  });
});

describe("Instellingen on a phone", () => {
  it("has no Starten met Windows", async () => {
    const real = api.appInfo;
    vi.spyOn(api, "appInfo").mockImplementation(async () => ({ ...(await real()), isPhone: true }));
    await openSettings();
    expect(screen.queryByLabelText("Starten met Windows")).toBeNull();
    expect(screen.getByLabelText("Melding bij nieuwe mail")).toBeTruthy();
  });

  it("has Starten met Windows on a PC", async () => {
    await openSettings();
    expect(screen.getByLabelText("Starten met Windows")).toBeTruthy();
  });
});

describe("the page the browser shows after signing in", () => {
  it("says to close the window on a PC and to go back to the app on a phone", () => {
    expect(oauthDonePage(false)).toBe("U bent ingelogd. U kunt dit venster sluiten en teruggaan naar Vorige Eeuw Mail.");
    expect(oauthDonePage(true)).toBe("U bent ingelogd. Open Vorige Eeuw Mail weer om verder te gaan.");
  });
});
