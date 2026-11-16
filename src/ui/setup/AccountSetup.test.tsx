import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { resetMockBackend } from "../../api/mock";
import { createSampleState } from "../../api/mock/data";

beforeEach(() => {
  resetMockBackend({ ...createSampleState(), accounts: [], folders: [], messages: [] });
});

afterEach(() => vi.restoreAllMocks());

async function fillAddress(name: string, email: string) {
  const user = userEvent.setup();
  render(<App />);
  await user.type(await screen.findByLabelText("Uw naam"), name);
  await user.type(screen.getByLabelText("Uw e-mailadres"), email);
  await user.click(screen.getByRole("button", { name: "Verder" }));
  return user;
}

describe("first run", () => {
  it("welcomes and asks for name and address first", async () => {
    render(<App />);
    expect(await screen.findByText("Welkom bij Vorige Eeuw Mail")).toBeTruthy();
    expect(screen.getByLabelText("Uw naam")).toBeTruthy();
    expect(screen.getByLabelText("Uw e-mailadres")).toBeTruthy();
    expect(screen.queryByLabelText("Wachtwoord van uw e-mail")).toBeNull();
  });
});

describe("password route", () => {
  it("asks for the password after the lookup and shows a refused login per errors.md", async () => {
    const user = await fillAddress("Ria", "ria@solcon.nl");
    await user.type(await screen.findByLabelText("Wachtwoord van uw e-mail"), "fout");
    await user.click(screen.getByRole("button", { name: "Postvak toevoegen" }));
    expect(await screen.findByText("Inloggen bij uw postvak is niet gelukt.")).toBeTruthy();
    expect(document.querySelector("pre.error-reply")?.textContent).toBe(
      "a1 NO [AUTHENTICATIONFAILED] Authentication failed.",
    );
  });

  it("adds the mailbox with a good password and opens the mail screen", async () => {
    const user = await fillAddress("Ria", "ria@solcon.nl");
    await user.type(await screen.findByLabelText("Wachtwoord van uw e-mail"), "goed");
    await user.click(screen.getByRole("button", { name: "Postvak toevoegen" }));
    expect(await screen.findByRole("navigation", { name: "Mappen" })).toBeTruthy();
  });

  it("asks before a password goes to a guessed host", async () => {
    const user = await fillAddress("Ria", "ria@eigen-domein.nl");
    expect(await screen.findByText(/mail\.hosting-voorbeeld\.example/)).toBeTruthy();
    expect(screen.queryByLabelText("Wachtwoord van uw e-mail")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Ja, doorgaan" }));
    expect(await screen.findByLabelText("Wachtwoord van uw e-mail")).toBeTruthy();
  });
});

describe("Google and Microsoft", () => {
  it("shows Inloggen met Google instead of a password, and waits for the browser", async () => {
    const spy = vi.spyOn(api, "addAccountOAuth");
    const user = await fillAddress("Ria", "ria@gmail.com");
    const button = await screen.findByRole("button", { name: "Inloggen met Google" });
    expect(screen.queryByLabelText("Wachtwoord van uw e-mail")).toBeNull();
    await user.click(button);
    expect(
      await screen.findByText("Log in via het venster van uw internetprogramma. Kom daarna hier terug."),
    ).toBeTruthy();
    expect(spy.mock.calls[0][3]).toBe(
      "U bent ingelogd. U kunt dit venster sluiten en teruggaan naar Vorige Eeuw Mail.",
    );
    expect(await screen.findByRole("navigation", { name: "Mappen" }, { timeout: 3000 })).toBeTruthy();
  });

  it("falls back to an app password when Google sign-in is not available", async () => {
    vi.spyOn(api, "oauthAvailable").mockResolvedValue(false);
    await fillAddress("Ria", "ria@gmail.com");
    expect(await screen.findByText("Gebruik een app-wachtwoord van Google.")).toBeTruthy();
    expect(screen.getByLabelText("Wachtwoord van uw e-mail")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Postvak toevoegen" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("says plainly that Microsoft mailboxes cannot be opened yet and blocks adding", async () => {
    const spy = vi.spyOn(api, "addAccount");
    await fillAddress("Ria", "ria@outlook.com");
    expect(await screen.findByText("Dit soort postvak kan het programma nog niet openen.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Inloggen met Microsoft" })).toBeNull();
    expect((screen.getByRole("button", { name: "Postvak toevoegen" }) as HTMLButtonElement).disabled).toBe(true);
    expect(spy).not.toHaveBeenCalled();
  });

  it("shows the sign-in error in the normal error box", async () => {
    vi.spyOn(api, "addAccountOAuth").mockRejectedValue({ code: "VEM-AUTH-LOGIN-01", detail: "timed out" });
    const user = await fillAddress("Ria", "ria@gmail.com");
    await user.click(await screen.findByRole("button", { name: "Inloggen met Google" }));
    expect(await screen.findByText("Inloggen met Google of Microsoft is niet afgemaakt.")).toBeTruthy();
    expect(screen.getByText("Code: VEM-AUTH-LOGIN-01")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Inloggen met Google" })).toBeTruthy();
  });
});

describe("shell texts", () => {
  it("sends the Dutch tray and notification texts once at start", async () => {
    const spy = vi.spyOn(api, "setShellTexts");
    render(<App />);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toEqual({
      open: "Vorige Eeuw Mail openen",
      quit: "Afsluiten",
      newMail: "Nieuwe e-mail",
      newMailMany: "{count} nieuwe e-mails",
      stillRunning: "Vorige Eeuw Mail blijft nieuwe mail ophalen. U vindt het rechtsonder bij de klok.",
    });
  });

  it("ignores a failure to hand them over", async () => {
    vi.spyOn(api, "setShellTexts").mockRejectedValue({ code: "VEM-APP-CMD-01" });
    render(<App />);
    expect(await screen.findByText("Welkom bij Vorige Eeuw Mail")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
