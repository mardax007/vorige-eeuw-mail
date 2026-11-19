import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "./App";

async function renderApp() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  return user;
}

describe("main screen", () => {
  it("shows the folder pane in Outlook's Dutch words", async () => {
    const user = await renderApp();
    const nav = screen.getByRole("navigation", { name: "Mappen" });
    const pane = within(nav);
    expect(pane.getByRole("button", { name: /^Alle postvakken/ })).toBeTruthy();
    // Two mailboxes and the old mail from Outlook, each with its own main folders, always shown.
    expect(pane.getAllByRole("button", { name: /^Postvak IN/ })).toHaveLength(3);
    expect(pane.getAllByRole("button", { name: /^Verzonden items/ })).toHaveLength(3);
    expect(pane.getAllByRole("button", { name: /^Verwijderde items/ })).toHaveLength(2);
    expect(pane.getByText("Oude mail uit Outlook")).toBeTruthy();
    expect(pane.queryByRole("button", { name: /^Concepten/ })).toBeNull();

    // Each mailbox has its own "Meer mappen (n)", which opens its other folders under it.
    const more = pane.getAllByRole("button", { name: /^Meer mappen \(\d+\)$/ });
    expect(more).toHaveLength(2);
    await user.click(more[0]);
    expect(pane.getAllByRole("button", { name: /^Concepten/ })).toHaveLength(1);
    expect(pane.getAllByRole("button", { name: /^Ongewenste e-mail/ })).toHaveLength(1);
    await user.click(more[1]);
    expect(pane.getAllByRole("button", { name: /^Concepten/ })).toHaveLength(2);
    expect(pane.queryByRole("button", { name: "Contactpersonen" })).toBeNull();
    expect(document.querySelector(".top-bar")?.textContent).toContain("Contactpersonen");
    expect(pane.getByText("Rekeningen")).toBeTruthy();
  });

  it("shows sender name and real address in the list", async () => {
    await renderApp();
    const row = await screen.findByRole("button", { name: /Koffie donderdag\?/ });
    expect(row.textContent).toContain("Anneke de Vries");
    expect(row.textContent).toContain("<anneke.devries@voorbeeld.nl>");
  });

  it("opens HTML mail only in a sandboxed frame with images blocked", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Nieuwsbrief november/ }));
    const frame = await screen.findByTitle("Inhoud van het bericht");
    expect(frame.getAttribute("sandbox")).toBe("allow-popups");
    expect(frame.getAttribute("srcdoc")).toContain("Content-Security-Policy");
    expect(frame.getAttribute("srcdoc")).not.toContain("https:;");
    for (const word of ["Beantwoorden", "Allen beantwoorden", "Doorsturen", "Verwijderen", "Verplaatsen", "Label", "Afdrukken"]) {
      expect(screen.getByRole("button", { name: word })).toBeTruthy();
    }
    await user.click(screen.getByRole("button", { name: "Afbeeldingen tonen" }));
    await waitFor(() => expect(screen.getByTitle("Inhoud van het bericht").getAttribute("srcdoc")).toContain("https:"));
  });

  it("shows the link warning in one calm sentence", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /bevestig uw gegevens/ }));
    expect(await screen.findByText(/toont www\.ing\.nl, maar gaat naar ing-veilig-inloggen\.example/)).toBeTruthy();
  });

  it("deletes without asking and can undo", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await user.click(await screen.findByRole("button", { name: "Verwijderen" }));
    expect(await screen.findByText("Het bericht is naar Verwijderde items gezet.")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("button", { name: /Koffie donderdag\?/ })).toBeNull());
    await user.click(screen.getByRole("button", { name: "Ongedaan maken" }));
    expect(await screen.findByRole("button", { name: /Koffie donderdag\?/ })).toBeTruthy();
  });

  it("searches while typing, with mailbox and folder named", async () => {
    const user = await renderApp();
    await user.click(screen.getByRole("button", { name: "Alle mail" }));
    await user.type(screen.getByLabelText("Zoeken"), "texel");
    const hit = await screen.findByRole("button", { name: /Uw reis naar Texel/ });
    expect(hit.textContent).toContain("Ria Jansen · Vakantie");
  });
});

describe("keyboard and layout", () => {
  it("deletes the open message with the Delete key", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Sleutel/ }));
    await screen.findByRole("button", { name: "Beantwoorden" });
    await user.keyboard("{Delete}");
    expect(await screen.findByText("Het bericht is naar Verwijderde items gezet.")).toBeTruthy();
  });

  it("opens the reply with Ctrl+R", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Sleutel/ }));
    await screen.findByRole("button", { name: "Beantwoorden" });
    await user.keyboard("{Control>}r{/Control}");
    expect(((await screen.findByLabelText("Onderwerp")) as HTMLInputElement).value).toBe("Re: Sleutel");
  });

  it("shows list and message side by side on a wide window", async () => {
    window.innerWidth = 1600;
    try {
      await renderApp();
      expect(await screen.findByText("Kies links een bericht om het te lezen.")).toBeTruthy();
    } finally {
      window.innerWidth = 1024;
    }
  });

  it("opens the message over the list with a Terug button on a narrow window", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Sleutel/ }));
    await user.click(await screen.findByRole("button", { name: /Terug/ }));
    expect(await screen.findByRole("button", { name: /Sleutel/ })).toBeTruthy();
  });
});

describe("sending", () => {
  it("waits before sending and Ongedaan maken brings the message back", async () => {
    const user = await renderApp();
    await user.click(screen.getByRole("button", { name: "Nieuwe e-mail" }));
    await user.type(screen.getByLabelText("Aan"), "wim.peters@voorbeeld.nl");
    await user.type(screen.getByLabelText("Onderwerp"), "Sleutel terug");
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    expect(await screen.findByText(/Wordt verstuurd over \d+ seconden\./)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Ongedaan maken" }));
    expect(((await screen.findByLabelText("Onderwerp")) as HTMLInputElement).value).toBe("Sleutel terug");
  });
});
