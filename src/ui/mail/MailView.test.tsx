import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { getMockBackend, resetMockBackend } from "../../api/mock";
import { createSampleState } from "../../api/mock/data";
import { AppProvider } from "../state/AppProvider";
import { ReadingPane } from "./ReadingPane";

async function renderApp() {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  return user;
}

afterEach(() => vi.restoreAllMocks());

const LOGIN_ERROR = { code: "VEM-IMAP-LOGIN-01", serverReply: "a1 NO [AUTHENTICATIONFAILED]", detail: null };

describe("sync errors", () => {
  it("shows every mailbox's error on Alle postvakken, each with the mailbox name", async () => {
    await renderApp();
    await waitFor(() => {
      getMockBackend().events.emit("sync:state", { accountId: "acc-gmail", busy: false, error: LOGIN_ERROR });
      expect(screen.getByText("Inloggen bij uw postvak is niet gelukt.")).toBeTruthy();
    });
    const box = document.querySelector(".sync-error") as HTMLElement;
    expect(within(box).getByRole("heading", { name: "Ria Jansen (Gmail)" })).toBeTruthy();
  });

  it("shows an error from before the screens opened (a failed fetch at start)", async () => {
    const state = createSampleState();
    const error = { code: "VEM-STORE-SECRET-01", serverReply: null, detail: "unprotect" };
    resetMockBackend({ ...state, accounts: state.accounts.map((a) => (a.id === "acc-gmail" ? { ...a, lastError: error } : a)) });
    await renderApp();
    expect(await screen.findByText("Het bewaarde wachtwoord kon niet gelezen worden.")).toBeTruthy();
  });
});

describe("a list that cannot be loaded", () => {
  it("says so instead of 'no messages', and Opnieuw proberen loads again", async () => {
    const real = api.listMessages;
    const spy = vi.spyOn(api, "listMessages").mockRejectedValue({ code: "VEM-STORE-READ-01" });
    const user = await renderApp();
    expect(await screen.findByText("De berichten konden niet geladen worden. Bovenaan staat wat er misging.")).toBeTruthy();
    expect(screen.queryByText("Er staan geen berichten in deze map.")).toBeNull();
    spy.mockImplementation(real);
    await user.click(screen.getByRole("button", { name: "Opnieuw proberen" }));
    expect(await screen.findByRole("button", { name: /Koffie donderdag\?/ })).toBeTruthy();
  });

  it("says a failed search failed instead of 'Niets gevonden'", async () => {
    vi.spyOn(api, "search").mockRejectedValue({ code: "VEM-STORE-READ-01" });
    const user = await renderApp();
    await user.type(screen.getByLabelText("Zoeken"), "texel");
    expect(await screen.findByText("Zoeken lukte niet. Bovenaan staat wat er misging.")).toBeTruthy();
    expect(screen.queryByText(/Niets gevonden/)).toBeNull();
  });
});

describe("a message that cannot be opened", () => {
  it("shows the error and a Terug button instead of waiting forever", async () => {
    vi.spyOn(api, "getMessage").mockRejectedValue({ code: "VEM-STORE-READ-01", detail: "message m-1" });
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    expect(await screen.findByText("Bewaarde mail kon niet gelezen worden.")).toBeTruthy();
    expect(screen.queryByText("Bericht wordt geopend…")).toBeNull();
    await user.click(screen.getByRole("button", { name: /Terug/ }));
    expect(await screen.findByRole("button", { name: /Koffie donderdag\?/ })).toBeTruthy();
  });

  it("still opens the message when marking it read fails", async () => {
    vi.spyOn(api, "setRead").mockRejectedValue({ code: "VEM-STORE-WRITE-01" });
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    expect(await screen.findByRole("button", { name: "Beantwoorden" })).toBeTruthy();
  });
});

describe("switching messages while one is still loading", () => {
  it("keeps the newer message on screen and does not mark the older one read", async () => {
    const real = api.getMessage;
    const late: Array<() => void> = [];
    let isFirst = true;
    const get = vi.spyOn(api, "getMessage").mockImplementation((id, remote) => {
      if (!isFirst) return real(id, remote);
      isFirst = false;
      return new Promise((resolve, reject) => {
        late.push(() => void real(id, remote).then(resolve, reject));
      });
    });
    const setRead = vi.spyOn(api, "setRead");
    const idOf = (subject: string) => getMockBackend().snapshot().messages.find((m) => m.subject === subject)?.id ?? "";
    const first = idOf("Foto's van de verjaardag");
    const pane = (id: string) => (
      <AppProvider>
        <ReadingPane messageId={id} onRemoved={vi.fn()} />
      </AppProvider>
    );
    const { rerender } = render(pane(first));
    await waitFor(() => expect(get).toHaveBeenCalledTimes(1));
    rerender(pane(idOf("Koffie donderdag?")));
    expect(await screen.findByRole("region", { name: "Koffie donderdag?" })).toBeTruthy();
    late.forEach((finish) => finish());
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.getByRole("region", { name: "Koffie donderdag?" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Foto's van de verjaardag" })).toBeNull();
    expect(setRead.mock.calls.some(([ids]) => ids.includes(first))).toBe(false);
  });
});

describe("message buttons", () => {
  it("names what Belangrijk will do", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await user.click(await screen.findByRole("button", { name: "☆ Belangrijk maken" }));
    expect(await screen.findByRole("button", { name: "★ Niet meer belangrijk" })).toBeTruthy();
  });

  it("takes the attachments along with Doorsturen", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Afvalkalender 2027/ }));
    await user.click(await screen.findByRole("button", { name: "Doorsturen" }));
    const subject = (await screen.findByLabelText("Onderwerp")) as HTMLInputElement;
    expect(subject.value).toBe("Fw: Afvalkalender 2027");
    expect(screen.getByText("afvalkalender-2027.pdf")).toBeTruthy();
  });

  it("says the attachments are being made ready, and takes no second tap meanwhile", async () => {
    let finish: (paths: string[]) => void = () => {};
    const get = vi.spyOn(api, "forwardAttachments").mockImplementation(
      () => new Promise<string[]>((resolve) => (finish = resolve)),
    );
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Afvalkalender 2027/ }));
    await user.click(await screen.findByRole("button", { name: "Doorsturen" }));
    expect(await screen.findByText("De bijlagen worden klaargezet om mee te sturen…")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Doorsturen" }) as HTMLButtonElement).disabled).toBe(true);
    expect(get).toHaveBeenCalledTimes(1);
    act(() => finish(["C:\\Temp\\afvalkalender-2027.pdf"]));
    expect(((await screen.findByLabelText("Onderwerp")) as HTMLInputElement).value).toBe("Fw: Afvalkalender 2027");
  });

  it("does not open Write without the attachments when they cannot be fetched", async () => {
    vi.spyOn(api, "forwardAttachments").mockRejectedValue({ code: "VEM-STORE-READ-01" });
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Afvalkalender 2027/ }));
    await user.click(await screen.findByRole("button", { name: "Doorsturen" }));
    expect(await screen.findByText("Bewaarde mail kon niet gelezen worden.")).toBeTruthy();
    expect(screen.queryByLabelText("Onderwerp")).toBeNull();
  });

  it("prints the message with Afdrukken on the PC", async () => {
    const print = vi.spyOn(window, "print").mockImplementation(() => {});
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await user.click(await screen.findByRole("button", { name: "Afdrukken" }));
    expect(print).toHaveBeenCalledTimes(1);
  });

  it("has no Afdrukken on a phone, whose WebView cannot print", async () => {
    const real = api.appInfo;
    vi.spyOn(api, "appInfo").mockImplementation(async () => ({ ...(await real()), isPhone: true }));
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await screen.findByRole("button", { name: "Beantwoorden" });
    expect(screen.queryByRole("button", { name: "Afdrukken" })).toBeNull();
  });

  it("has no Verwijderen inside Verwijderde items", async () => {
    const user = await renderApp();
    const nav = within(screen.getByRole("navigation", { name: "Mappen" }));
    await user.click(nav.getAllByRole("button", { name: /^Verwijderde items/ })[0]);
    await user.click(await screen.findByRole("button", { name: /U hebt gewonnen!/ }));
    await screen.findByRole("button", { name: "Beantwoorden" });
    expect(screen.queryByRole("button", { name: "Verwijderen" })).toBeNull();
  });

  it("says where a moved message went", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await user.click(await screen.findByRole("button", { name: "Verplaatsen" }));
    const menu = within(screen.getByRole("group", { name: "Verplaatsen naar:" }));
    await user.click(menu.getByRole("button", { name: "Vakantie" }));
    expect(await screen.findByText("Het bericht is verplaatst naar Vakantie.")).toBeTruthy();
  });

  it("shows an invitation readably, pointing at its calendar file", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Herinnering afspraak griepprik/ }));
    const box = within(await screen.findByRole("region", { name: "Uitnodiging" }));
    expect(box.getByText("Griepprik")).toBeTruthy();
    expect(box.getByText(/dinsdag 24 november 2026, \d\d:00 tot \d\d:00/)).toBeTruthy();
    expect(box.getByText("Huisartsenpraktijk De Linde, Kerkstraat 4")).toBeTruthy();
    expect(box.getByText(/open dan de bijlage uitnodiging\.ics/)).toBeTruthy();
  });

  it("moves a message to Ongewenste e-mail and back to Postvak IN", async () => {
    const user = await renderApp();
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await user.click(await screen.findByRole("button", { name: "Ongewenste e-mail" }));
    expect(await screen.findByText("Het bericht is verplaatst naar Ongewenste e-mail.")).toBeTruthy();
    const nav = within(screen.getByRole("navigation", { name: "Mappen" }));
    // The first mailbox's own "Meer mappen" opens its other folders right under it.
    const more = nav.getAllByRole("button", { name: /^Meer mappen/ })[0];
    expect(more.getAttribute("aria-expanded")).toBe("false");
    await user.click(more);
    expect(more.getAttribute("aria-expanded")).toBe("true");
    await user.click(nav.getAllByRole("button", { name: /^Ongewenste e-mail/ })[0]);
    await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
    await user.click(await screen.findByRole("button", { name: "Geen ongewenste e-mail" }));
    expect(await screen.findByText("Het bericht is verplaatst naar Postvak IN.")).toBeTruthy();
  });
});

describe("search", () => {
  it("ends when a folder is picked, also the same one again", async () => {
    const user = await renderApp();
    const box = screen.getByLabelText("Zoeken") as HTMLInputElement;
    await user.type(box, "texel");
    await screen.findByRole("heading", { name: "Zoekresultaten" });
    const nav = within(screen.getByRole("navigation", { name: "Mappen" }));
    await user.click(nav.getByRole("button", { name: /^Alle postvakken/ }));
    await waitFor(() => expect(box.value).toBe(""));
    expect(await screen.findByRole("heading", { name: "Alle postvakken" })).toBeTruthy();
  });
});

describe("on a phone", () => {
  it("shows one part at a time: list, Mappen, back to the list, the message with Terug", async () => {
    const width = window.innerWidth;
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 375 });
    try {
      const user = userEvent.setup();
      render(<App />);
      await screen.findByRole("button", { name: /Koffie donderdag\?/ });
      expect(screen.queryByRole("navigation", { name: "Mappen" })).toBeNull();

      await user.click(screen.getByRole("button", { name: "Mappen" }));
      await screen.findByRole("navigation", { name: "Mappen" });
      expect(screen.queryByRole("button", { name: /Koffie donderdag\?/ })).toBeNull();
      await user.click(screen.getByRole("button", { name: "Terug naar de berichten" }));
      expect(await screen.findByRole("button", { name: /Koffie donderdag\?/ })).toBeTruthy();

      await user.click(screen.getByRole("button", { name: "Mappen" }));
      await user.click(within(await screen.findByRole("navigation", { name: "Mappen" })).getByRole("button", { name: /^Alle postvakken/ }));
      expect(await screen.findByRole("heading", { name: "Alle postvakken" })).toBeTruthy();
      expect(screen.queryByRole("navigation", { name: "Mappen" })).toBeNull();

      await user.click(await screen.findByRole("button", { name: /Koffie donderdag\?/ }));
      expect(await screen.findByRole("button", { name: /Terug/ })).toBeTruthy();
      expect(screen.queryByRole("button", { name: "Mappen" })).toBeNull();
    } finally {
      Object.defineProperty(window, "innerWidth", { configurable: true, value: width });
    }
  });
});
