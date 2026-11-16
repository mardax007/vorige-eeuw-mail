import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { resetMockBackend } from "../../api/mock";
import { createSampleState } from "../../api/mock/data";

afterEach(() => vi.restoreAllMocks());

async function openImport() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  await user.click(screen.getByRole("button", { name: "Meer mappen" }));
  await user.click(screen.getByRole("button", { name: "Oude mail ophalen" }));
  await screen.findByRole("heading", { name: "Oude mail ophalen" });
  return user;
}

function accountRow(name: string | RegExp): HTMLElement {
  const label = screen.getAllByText(name).find((el) => el.closest("li.attachment-row"));
  return label?.closest("li") as HTMLElement;
}

describe("Uit een ander postvak", () => {
  it("copies an old mailbox into another one, never offering the old one as target", async () => {
    const spy = vi.spyOn(api, "importFromAccount");
    const user = await openImport();
    expect(screen.getByText(/Het oude postvak wordt daarbij nooit veranderd/)).toBeTruthy();
    await user.click(within(accountRow("Ria Jansen (Gmail)")).getByRole("button", { name: "Kiezen" }));
    expect(screen.getByText("Gekozen: alle mail uit Ria Jansen (Gmail)")).toBeTruthy();
    const where = within(screen.getByRole("group", { name: "Waar moet de mail heen?" }));
    await user.click(where.getByLabelText("In het postvak Ria Jansen zetten"));
    expect(where.queryByRole("combobox")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Ophalen" }));
    expect(spy).toHaveBeenCalledWith("acc-gmail", { kind: "mailbox", accountId: "acc-solcon" });
    expect((screen.getByRole("button", { name: "Ophalen" }) as HTMLButtonElement).disabled).toBe(true);
    expect(await screen.findByText(/^Klaar\. 500 berichten opgehaald, 1 overgeslagen\.$/, undefined, { timeout: 5000 })).toBeTruthy();
    expect(screen.getByText("12 berichten stonden er al en zijn niet nog eens neergezet.")).toBeTruthy();
    expect(screen.getByText("Details:")).toBeTruthy();
    expect(document.querySelector(".import-progress pre")?.textContent).toBe("Archief/2009: SELECT failed: NO Mailbox is locked");
    expect(screen.getByRole("button", { name: "Lijst kopiëren" })).toBeTruthy();
  });

  it("asks to add the old mailbox first when there is only one", async () => {
    const state = createSampleState();
    resetMockBackend({ ...state, accounts: state.accounts.filter((a) => a.id !== "acc-gmail") });
    const user = await openImport();
    expect(screen.getByText(/Voeg het oude postvak eerst toe/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Postvak toevoegen" }));
    expect(await screen.findByRole("heading", { name: "Postvak toevoegen" })).toBeTruthy();
  });
});

describe("a running import", () => {
  it("shows again after leaving the page, and Ophalen stays off until it is done", async () => {
    const user = await openImport();
    await user.click(within(accountRow(/^Outlook.pst/)).getByRole("button", { name: "Kiezen" }));
    await user.click(screen.getByRole("button", { name: "Ophalen" }));
    await screen.findByText(/van 500 berichten/);
    await user.click(screen.getByRole("button", { name: /Terug naar de mail/ }));
    await user.click(await screen.findByRole("button", { name: "Meer mappen" }));
    await user.click(screen.getByRole("button", { name: "Oude mail ophalen" }));
    await screen.findByRole("heading", { name: "Oude mail ophalen" });
    expect(await screen.findByText(/van 500 berichten/)).toBeTruthy();
    await user.click(within(accountRow(/^Outlook.pst/)).getByRole("button", { name: "Kiezen" }));
    expect((screen.getByRole("button", { name: "Ophalen" }) as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => expect(screen.getByText(/^Klaar\./)).toBeTruthy(), { timeout: 5000 });
    expect((screen.getByRole("button", { name: "Ophalen" }) as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("an Outlook file with a password", () => {
  it("asks for the password and says under the field when it is wrong", async () => {
    const spy = vi.spyOn(api, "startImport");
    const user = await openImport();
    await user.click(within(accountRow(/^Archief 2012\.pst/)).getByRole("button", { name: "Kiezen" }));
    const start = screen.getByRole("button", { name: "Ophalen" }) as HTMLButtonElement;
    expect(start.disabled).toBe(true);
    const field = screen.getByLabelText("Wachtwoord van het Outlook-bestand");
    await user.type(field, "fout");
    await user.click(start);
    expect(await screen.findByText(/en dat wachtwoord klopt niet\..*\(VEM-IMPORT-READ-05\)/)).toBeTruthy();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    await user.clear(field);
    await user.type(field, "geheim");
    expect(field.getAttribute("aria-invalid")).toBeNull();
    await user.click(start);
    expect(spy).toHaveBeenLastCalledWith(expect.objectContaining({ label: "Archief 2012.pst" }), { kind: "local" }, "geheim");
    expect(await screen.findByText(/^Klaar\./, undefined, { timeout: 5000 })).toBeTruthy();
  });

  it("asks nothing for a file without one", async () => {
    const user = await openImport();
    await user.click(within(accountRow(/^Outlook\.pst/)).getByRole("button", { name: "Kiezen" }));
    expect(screen.queryByLabelText("Wachtwoord van het Outlook-bestand")).toBeNull();
    expect((screen.getByRole("button", { name: "Ophalen" }) as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("Contactpersonen uit Windows Live Mail", () => {
  it("explains the export, reads the cards and opens Contactpersonen", async () => {
    const spy = vi.spyOn(api, "importContactsFile");
    const user = await openImport();
    expect(screen.getByText("Klik bovenaan op Exporteren en kies Visitekaartje (.VCF).")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Map met visitekaartjes kiezen" }));
    expect(await screen.findByText(/Toegevoegd aan Contactpersonen: 1\. Stonden er al \(en zijn niet veranderd\): 1\./)).toBeTruthy();
    expect(spy).toHaveBeenCalledWith(true);
    await user.click(screen.getByRole("button", { name: "Contactpersonen openen" }));
    expect(await screen.findByText("Truus de Wit")).toBeTruthy();
  });
});

describe("what the import says at the end", () => {
  it("names the contacts from Outlook", async () => {
    const user = await openImport();
    await user.click(within(accountRow(/^Outlook.pst/)).getByRole("button", { name: "Kiezen" }));
    await user.click(screen.getByRole("button", { name: "Ophalen" }));
    expect(await screen.findByText("Uit Outlook toegevoegd aan Contactpersonen: 3.", undefined, { timeout: 5000 })).toBeTruthy();
    expect(screen.queryByText(/staan alle opgehaalde berichten op gelezen/)).toBeNull();
  });

  it("says Windows Live Mail mail came in as read", async () => {
    const user = await openImport();
    await user.click(screen.getByRole("button", { name: "Windows Live Mail" }));
    await user.click(await screen.findByRole("button", { name: "Ophalen" }));
    expect(await screen.findByText(/staan alle opgehaalde berichten op gelezen/, undefined, { timeout: 5000 })).toBeTruthy();
  });
});
