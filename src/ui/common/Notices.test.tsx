import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { getMockBackend, resetMockBackend } from "../../api/mock";
import { createSampleState } from "../../api/mock/data";
import type { OutboxItem } from "../../api/types";
import { sendPhase } from "./Notices";

const item = (state: OutboxItem["state"]): OutboxItem => ({ id: "o-1", subject: "x", sendAt: "", state, error: null });

describe("sendPhase", () => {
  it("counts down while waiting, and says it is sending once the wait is over", () => {
    expect(sendPhase(item("waiting"), true, 5)).toBe("countdown");
    expect(sendPhase(item("waiting"), true, 0)).toBe("sending");
    expect(sendPhase(item("sending"), true, 0)).toBe("sending");
  });

  it("says failed for a failed item, also before the wait would be over", () => {
    expect(sendPhase(item("failed"), true, 0)).toBe("failed");
    expect(sendPhase(item("failed"), true, 4)).toBe("failed");
  });

  it("says sent only when the item was seen in Postvak UIT and is gone from it", () => {
    expect(sendPhase(undefined, true, 0)).toBe("sent");
    expect(sendPhase(undefined, false, 0)).toBe("sending");
    expect(sendPhase(undefined, false, 3)).toBe("countdown");
  });
});

function startWithoutWait() {
  const state = createSampleState();
  resetMockBackend({ ...state, settings: { ...state.settings, undoSendSeconds: 0 } });
}

async function sendTo(address: string) {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  await user.click(screen.getByRole("button", { name: "Nieuwe e-mail" }));
  await user.type(screen.getByLabelText("Aan"), address);
  await user.type(screen.getByLabelText("Onderwerp"), "Proef");
  await user.click(screen.getByRole("button", { name: "Versturen" }));
  return user;
}

afterEach(() => vi.restoreAllMocks());

describe("send bar", () => {
  it("keeps a failed message in view with Postvak UIT openen, and never says it was sent", async () => {
    startWithoutWait();
    const user = await sendTo("weigert@voorbeeld.nl");
    expect(await screen.findByText("Het bericht is niet verstuurd. Het staat in Postvak UIT.")).toBeTruthy();
    expect(screen.queryByText("Het bericht is verstuurd.")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Postvak UIT openen" }));
    const list = await screen.findByRole("heading", { name: "Postvak UIT" });
    expect(within(list.closest("section") as HTMLElement).getByText(/Niet verstuurd/)).toBeTruthy();
  });

  it("shows the server's exact reply in Postvak UIT and gives the message back to change it", async () => {
    startWithoutWait();
    const user = await sendTo("weigert@voorbeeld.nl");
    await user.click(await screen.findByRole("button", { name: "Postvak UIT openen" }));
    const list = (await screen.findByRole("heading", { name: "Postvak UIT" })).closest("section") as HTMLElement;
    const reply = list.querySelector("pre.error-reply");
    expect(reply?.textContent).toBe(
      "550 5.1.1 <weigert@voorbeeld.nl>: Recipient address rejected: User unknown in virtual mailbox table",
    );
    await user.click(within(list).getByRole("button", { name: "Terughalen en aanpassen" }));
    await waitFor(() => expect((screen.getByLabelText("Onderwerp") as HTMLInputElement).value).toBe("Proef"));
    expect(await api.listOutbox()).toEqual([]);
  });

  it("says it was sent once the message has left Postvak UIT", async () => {
    startWithoutWait();
    await sendTo("wim.peters@voorbeeld.nl");
    expect(await screen.findByText("Het bericht is verstuurd.")).toBeTruthy();
  });

  it("does not say sent when Postvak UIT is read again before the message shows up in it", async () => {
    const waiting: OutboxItem = {
      id: "o-later", subject: "Proef", sendAt: new Date(Date.now() + 30_000).toISOString(), state: "waiting", error: null,
    };
    vi.spyOn(api, "sendMessage").mockResolvedValue(waiting);
    await sendTo("wim.peters@voorbeeld.nl");
    expect(await screen.findByText(/Wordt verstuurd over/)).toBeTruthy();
    act(() => getMockBackend().events.emit("outbox:changed", []));
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.queryByText("Het bericht is verstuurd.")).toBeNull();
    expect(screen.getByText(/Wordt verstuurd over/)).toBeTruthy();
  });

  it("takes a message back once when Ongedaan maken is pressed twice, without an error", async () => {
    const real = api.cancelSend;
    const cancel = vi.spyOn(api, "cancelSend").mockImplementation(async (id) => {
      await new Promise((r) => setTimeout(r, 50));
      return real(id);
    });
    const user = await sendTo("wim.peters@voorbeeld.nl");
    const undo = await screen.findByRole("button", { name: "Ongedaan maken" });
    await user.click(undo);
    await user.click(undo);
    expect((undo as HTMLButtonElement).disabled).toBe(true);
    expect(await screen.findByLabelText("Onderwerp")).toBeTruthy();
    await waitFor(() => expect((screen.getByLabelText("Onderwerp") as HTMLInputElement).value).toBe("Proef"));
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("Bijwerken on a phone", () => {
  const offer = { version: "0.2.0", notes: ["Zoeken gaat sneller."] };

  it("offers a newer version and opens Android's install screen", async () => {
    vi.spyOn(api, "updateOffer").mockResolvedValue(offer);
    const install = vi.spyOn(api, "installUpdate").mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText("Er is een nieuwe versie van Vorige Eeuw Mail: 0.2.0.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Bijwerken" }));
    expect(install).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/Android vraagt nu of u wilt bijwerken/)).toBeTruthy();
  });

  it("shows an update found while the app is open", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Mappen" });
    expect(screen.queryByRole("button", { name: "Bijwerken" })).toBeNull();
    await waitFor(() => {
      getMockBackend().events.emit("update:available", offer);
      expect(screen.getByRole("button", { name: "Bijwerken" })).toBeTruthy();
    });
  });

  it("shows the error and offers Bijwerken again when fetching fails", async () => {
    vi.spyOn(api, "updateOffer").mockResolvedValue(offer);
    vi.spyOn(api, "installUpdate").mockRejectedValue({ code: "VEM-UPDATE-GET-01", detail: "signature does not match" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Bijwerken" }));
    expect(await screen.findByText("De nieuwe versie kon niet opgehaald worden.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Bijwerken" })).toBeTruthy();
  });
});

describe("mail from the PC on the phone", () => {
  it("counts while it comes in, then says it is there", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Mappen" });
    await waitFor(() => {
      getMockBackend().events.emit("lan:phone", { kind: "progress", done: 1200, total: 5000 });
      expect(screen.getByText("Mail van de computer wordt op deze telefoon gezet: 1.200 van 5.000.")).toBeTruthy();
    });
    act(() => getMockBackend().events.emit("lan:phone", { kind: "done", received: 5000, removed: 0 }));
    expect(screen.getByText("De mail van de computer staat nu ook op deze telefoon.")).toBeTruthy();
  });

  it("says only a full phone as an error", async () => {
    render(<App />);
    await screen.findByRole("navigation", { name: "Mappen" });
    await waitFor(() => {
      getMockBackend().events.emit("lan:phone", { kind: "failed", error: { code: "VEM-LAN-SPACE-01" } });
      expect(screen.getByText("Er is niet genoeg ruimte op deze telefoon voor de mail van de computer.")).toBeTruthy();
    });
  });
});
