import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "../../App";
import { resetMockBackend } from "../../api/mock";
import { createSampleState } from "../../api/mock/data";

async function openContacts() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  await user.click(screen.getByRole("button", { name: "Contactpersonen" }));
  await screen.findByRole("heading", { name: "Contactpersonen" });
  return user;
}

describe("Contactpersonen", () => {
  it("saves the address book as a file and adds people from one", async () => {
    const user = await openContacts();
    await user.click(screen.getByRole("button", { name: "Opslaan als bestand" }));
    expect(await screen.findByText("Opgeslagen in C:\\Users\\Ria\\Documents\\Contactpersonen.vcf")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Truus de Wit/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Toevoegen uit een bestand" }));
    expect(await screen.findByRole("button", { name: /Truus de Wit/ })).toBeTruthy();
    expect(screen.getByText("Toegevoegd aan Contactpersonen: 1.")).toBeTruthy();
  });

  it("has one back button to the mail on the list", async () => {
    await openContacts();
    expect(screen.getAllByRole("button", { name: /Terug/ }).map((b) => b.textContent)).toEqual(["← Terug naar de mail"]);
  });

  it("has only Terug naar Contactpersonen while a contact is open", async () => {
    const user = await openContacts();
    await user.click(await screen.findByRole("button", { name: /Anneke de Vries/ }));
    await screen.findByRole("heading", { name: "Anneke de Vries" });
    expect(screen.getAllByRole("button", { name: /Terug/ }).map((b) => b.textContent)).toEqual(["← Terug naar Contactpersonen"]);
    await user.click(screen.getByRole("button", { name: /Terug naar Contactpersonen/ }));
    expect(await screen.findByLabelText("Zoek een naam of adres")).toBeTruthy();
  });

  it("offers no back button that drops typed text while editing; Annuleren goes back to the contact", async () => {
    const user = await openContacts();
    await user.click(await screen.findByRole("button", { name: /Anneke de Vries/ }));
    await user.click(await screen.findByRole("button", { name: "Wijzigen" }));
    expect(screen.queryByRole("button", { name: /Terug/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Annuleren" }));
    expect(await screen.findByRole("button", { name: "E-mail sturen" })).toBeTruthy();
  });

  it("puts a wrong address under its own field and goes there", async () => {
    const user = await openContacts();
    await user.click(screen.getByRole("button", { name: "Nieuwe contactpersoon" }));
    await user.type(screen.getByLabelText("Naam"), "Kees");
    const address = screen.getByLabelText("E-mailadres 1");
    await user.type(address, "kees@");
    await user.click(screen.getByRole("button", { name: "Opslaan" }));
    const problem = await screen.findByRole("alert");
    expect(address.getAttribute("aria-describedby")).toBe(problem.id);
    await new Promise((r) => requestAnimationFrame(r));
    expect(document.activeElement).toBe(address);
    await user.type(address, "x.nl");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("tells a search without match apart from an empty list", async () => {
    const user = await openContacts();
    await user.type(screen.getByLabelText("Zoek een naam of adres"), "xyz");
    expect(await screen.findByText("Niemand gevonden voor “xyz”.")).toBeTruthy();
    expect(screen.queryByText(/Nog geen contactpersonen/)).toBeNull();
  });

  it("explains how to add the first contact", async () => {
    resetMockBackend({ ...createSampleState(), contacts: [] });
    await openContacts();
    expect(await screen.findByText("Nog geen contactpersonen. Druk op Nieuwe contactpersoon om iemand toe te voegen.")).toBeTruthy();
  });
});
