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
