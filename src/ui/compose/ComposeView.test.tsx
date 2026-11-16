import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import type { Draft } from "../../api/types";
import { AppProvider } from "../state/AppProvider";
import { ComposeView } from "./ComposeView";

const draft: Draft = {
  id: null, accountId: "acc-solcon", to: [], cc: [], bcc: [], subject: "", bodyText: "",
  attachments: [], inReplyTo: null, references: [], shrinkPhotos: true,
};

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

  it("asks for an address before sending", async () => {
    const spy = vi.spyOn(api, "sendMessage");
    const { user } = renderCompose();
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    expect(screen.getByRole("alert").textContent).toBe("Vul bij Aan minstens één adres in.");
    expect(spy).not.toHaveBeenCalled();
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
