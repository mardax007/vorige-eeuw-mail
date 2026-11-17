import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import { getMockBackend } from "../../api/mock";
import type { Draft } from "../../api/types";
import { AppProvider } from "../state/AppProvider";
import { ComposeView } from "./ComposeView";

const draft: Draft = {
  id: null, accountId: "acc-solcon", to: [], cc: [], bcc: [], subject: "", bodyText: "",
  attachments: [], inReplyTo: null, references: [], shrinkPhotos: true,
};

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Drafts left in Concepten, in the core's list and as messages in a drafts folder. */
function draftsLeft(): number {
  const s = getMockBackend().snapshot();
  const draftFolders = new Set(s.folders.filter((f) => f.role === "drafts").map((f) => f.id));
  return Object.keys(s.drafts).length + s.messages.filter((m) => draftFolders.has(m.folderId)).length;
}

function renderCompose(onClose = vi.fn(), autosaveMs = 60_000) {
  render(
    <AppProvider>
      <ComposeView initial={draft} autosaveMs={autosaveMs} onClose={onClose} />
    </AppProvider>,
  );
  return { user: userEvent.setup(), onClose };
}

afterEach(() => vi.restoreAllMocks());

describe("ComposeView", () => {
  it("shows the Outlook words and keeps CC behind a button", async () => {
    const { user } = renderCompose();
    expect(screen.getByLabelText("Aan")).toBeTruthy();
    expect(screen.getByLabelText("Onderwerp")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Versturen" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Bijlage toevoegen" })).toBeTruthy();
    expect(screen.getByLabelText("Foto's kleiner maken")).toBeTruthy();
    expect(screen.queryByLabelText("CC")).toBeNull();
    await user.click(screen.getByRole("button", { name: "CC toevoegen" }));
    expect(screen.getByLabelText("CC")).toBeTruthy();
  });

  it("autosaves to Concepten while typing", async () => {
    const spy = vi.spyOn(api, "saveDraft");
    const { user } = renderCompose(vi.fn(), 50);
    await user.type(screen.getByLabelText("Bericht"), "Lieve Sanne");
    await waitFor(() => {
      expect(spy).toHaveBeenCalled();
      expect(spy.mock.calls[spy.mock.calls.length - 1][0].bodyText).toContain("Lieve Sanne");
    });
  });

  it("does not save an empty new message", async () => {
    const spy = vi.spyOn(api, "saveDraft");
    const { user, onClose } = renderCompose(vi.fn(), 20);
    await new Promise((r) => setTimeout(r, 80));
    await user.click(screen.getByRole("button", { name: "Sluiten" }));
    expect(onClose).toHaveBeenCalled();
    expect(spy).not.toHaveBeenCalled();
  });

  it("saves on Sluiten so no text is lost", async () => {
    const spy = vi.spyOn(api, "saveDraft");
    const { user, onClose } = renderCompose();
    await user.type(screen.getByLabelText("Onderwerp"), "Kerst");
    await user.click(screen.getByRole("button", { name: "Sluiten" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0].subject).toBe("Kerst");
  });

  it("asks for an address before sending, under the Aan field", async () => {
    const spy = vi.spyOn(api, "sendMessage");
    const { user } = renderCompose();
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    expect(screen.getByRole("alert").textContent).toBe("Vul bij Aan minstens één adres in.");
    const to = screen.getByLabelText("Aan");
    expect(to.getAttribute("aria-invalid")).toBe("true");
    expect(document.getElementById(to.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
      "Vul bij Aan minstens één adres in.",
    );
    expect(document.activeElement).toBe(to);
    expect(spy).not.toHaveBeenCalled();
  });

  it("shows a wrong CC address under the CC field and focuses it", async () => {
    const { user } = renderCompose();
    await user.type(screen.getByLabelText("Aan"), "anneke.devries@voorbeeld.nl");
    await user.click(screen.getByRole("button", { name: "CC toevoegen" }));
    await user.type(screen.getByLabelText("CC"), "kees@");
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    const cc = screen.getByLabelText("CC");
    expect(cc.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByLabelText("Aan").getAttribute("aria-invalid")).toBeNull();
    expect(screen.getByRole("alert").textContent).toBe("Dit adres klopt niet: kees@");
    expect(document.activeElement).toBe(cc);
  });

  it("stays open with the text when the last save failed", async () => {
    vi.spyOn(api, "saveDraft").mockRejectedValue({ code: "VEM-STORE-WRITE-01", detail: "disk full" });
    const { user, onClose } = renderCompose();
    await user.type(screen.getByLabelText("Onderwerp"), "Kerst");
    await user.click(screen.getByRole("button", { name: "Sluiten" }));
    expect(await screen.findByText("Het programma kon niets opslaan op deze computer.")).toBeTruthy();
    expect(screen.getByText(/Daarom blijft het open/)).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
    expect((screen.getByLabelText("Onderwerp") as HTMLInputElement).value).toBe("Kerst");
  });

  it("sends once when Versturen is pressed twice", async () => {
    let finish: (() => void) | null = null;
    const real = api.sendMessage;
    const spy = vi.spyOn(api, "sendMessage").mockImplementation(
      (d) => new Promise((resolve) => { finish = () => void real(d).then(resolve); }),
    );
    const { user } = renderCompose();
    await user.type(screen.getByLabelText("Aan"), "wim.peters@voorbeeld.nl");
    const send = screen.getByRole("button", { name: "Versturen" });
    await user.click(send);
    await user.click(send);
    expect(spy).toHaveBeenCalledTimes(1);
    expect((send as HTMLButtonElement).disabled).toBe(true);
    (finish as (() => void) | null)?.();
  });

  it("leaves no draft behind when a save is still running at Versturen", async () => {
    const real = api.saveDraft;
    const save = vi.spyOn(api, "saveDraft").mockImplementation(async (d) => {
      await wait(120);
      return real(d);
    });
    const send = vi.spyOn(api, "sendMessage");
    const { user } = renderCompose(vi.fn(), 30);
    await user.type(screen.getByLabelText("Aan"), "wim.peters@voorbeeld.nl");
    await waitFor(() => expect(save).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    await waitFor(() => expect(send).toHaveBeenCalledTimes(1));
    await wait(300);
    expect(send.mock.calls[0][0].id).not.toBeNull();
    expect(draftsLeft()).toBe(0);
  });

  it("does not save to Concepten while Versturen is running", async () => {
    const real = api.sendMessage;
    let finish: (() => void) | null = null;
    const send = vi.spyOn(api, "sendMessage").mockImplementation(
      (d) => new Promise((resolve) => { finish = () => void real(d).then(resolve); }),
    );
    const { user } = renderCompose(vi.fn(), 30);
    await user.type(screen.getByLabelText("Aan"), "wim.peters@voorbeeld.nl");
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    await waitFor(() => expect(send).toHaveBeenCalledTimes(1));
    const save = vi.spyOn(api, "saveDraft");
    await wait(150);
    expect(save).not.toHaveBeenCalled();
    (finish as (() => void) | null)?.();
    await wait(50);
    expect(draftsLeft()).toBe(0);
  });

  it("saves again after Versturen did not work", async () => {
    vi.spyOn(api, "sendMessage").mockRejectedValue({ code: "VEM-APP-CMD-01" });
    const { user } = renderCompose(vi.fn(), 30);
    await user.type(screen.getByLabelText("Aan"), "wim.peters@voorbeeld.nl");
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Versturen" }) as HTMLButtonElement).disabled).toBe(false));
    const save = vi.spyOn(api, "saveDraft");
    await user.type(screen.getByLabelText("Onderwerp"), "Kerst");
    await waitFor(() => expect(save).toHaveBeenCalled());
  });

  it("closes once when Sluiten is pressed twice", async () => {
    const real = api.saveDraft;
    const save = vi.spyOn(api, "saveDraft").mockImplementation(async (d) => {
      await wait(80);
      return real(d);
    });
    const { user, onClose } = renderCompose();
    await user.type(screen.getByLabelText("Onderwerp"), "Kerst");
    const close = screen.getByRole("button", { name: "Sluiten" });
    await user.click(close);
    await user.click(close);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    await wait(100);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("keeps a fixed place for the saved line, also while typing", async () => {
    const { user } = renderCompose(vi.fn(), 30);
    const note = document.querySelector(".compose-header .save-note");
    expect(note?.getAttribute("role")).toBe("status");
    await user.type(screen.getByLabelText("Onderwerp"), "Kerst");
    await waitFor(() => expect(note?.textContent).toBe("Opgeslagen in Concepten."));
    await user.type(screen.getByLabelText("Onderwerp"), "!");
    expect(note?.textContent).toBe("Opgeslagen in Concepten.");
    expect(document.querySelector(".compose-header .save-note")).toBe(note);
  });

  it("asks once before sending to more than ten people", async () => {
    const spy = vi.spyOn(api, "sendMessage");
    const { user } = renderCompose();
    const many = Array.from({ length: 11 }, (_, i) => `p${i}@voorbeeld.nl`).join(", ");
    await user.click(screen.getByLabelText("Aan"));
    await user.paste(many);
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    expect(await screen.findByText("U stuurt dit bericht naar 11 mensen. Is dat de bedoeling?")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Ja, versturen" }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
  });
});

describe("the heading", () => {
  it("says Nieuwe e-mail for a new mail and Antwoord schrijven for a reply", async () => {
    const { unmount } = render(
      <AppProvider>
        <ComposeView initial={draft} autosaveMs={60_000} onClose={vi.fn()} />
      </AppProvider>,
    );
    expect(await screen.findByRole("heading", { name: "Nieuwe e-mail" })).toBeTruthy();
    unmount();
    render(
      <AppProvider>
        <ComposeView initial={{ ...draft, subject: "Re: Koffie", inReplyTo: "<a@b>" }} autosaveMs={60_000} onClose={vi.fn()} />
      </AppProvider>,
    );
    expect(await screen.findByRole("heading", { name: "Antwoord schrijven" })).toBeTruthy();
  });
});
