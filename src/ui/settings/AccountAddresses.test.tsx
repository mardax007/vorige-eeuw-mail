import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../../App";
import { api } from "../../api/client";
import { resetMockBackend } from "../../api/mock";
import { replyFromAddress } from "../compose/prefill";

beforeEach(() => resetMockBackend());
afterEach(() => vi.restoreAllMocks());

const FAMILY = "familie@jansen-voorbeeld.nl";

function accountBlock(name: string): HTMLElement {
  const heading = screen.getAllByRole("heading", { level: 4 }).find((h) => h.textContent?.startsWith(`${name} `));
  return heading?.closest(".account-settings") as HTMLElement;
}

async function addAddress(address: string) {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("navigation", { name: "Mappen" });
  await user.click(screen.getByRole("button", { name: "Instellingen" }));
  await screen.findByRole("heading", { name: "Instellingen" });
  const block = within(accountBlock("Ria Jansen"));
  await user.type(block.getByLabelText("Nieuw adres"), address);
  await user.click(block.getByRole("button", { name: "Toevoegen" }));
  return { user, block };
}

describe("Andere adressen in one mailbox (decision 32)", () => {
  it("refuses a wrong address and one the mailbox already has", async () => {
    const save = vi.spyOn(api, "setAccountAddresses");
    const { user, block } = await addAddress("geen-adres");
    expect(block.getByRole("alert").textContent).toBe("Dit adres klopt niet: geen-adres");
    await user.clear(block.getByLabelText("Nieuw adres"));
    await user.type(block.getByLabelText("Nieuw adres"), "Ria.Jansen@solcon.nl");
    await user.click(block.getByRole("button", { name: "Toevoegen" }));
    expect(block.getByRole("alert").textContent).toBe("Dit adres hoort al bij dit postvak: ria.jansen@solcon.nl");
    expect(save).not.toHaveBeenCalled();
  });

  it("gives the domain its own Postvak IN and answers from the address the mail came to", async () => {
    const { user, block } = await addAddress("Familie@Jansen-Voorbeeld.nl");
    expect(await block.findByText(FAMILY)).toBeTruthy();

    await user.click(screen.getByRole("button", { name: /Terug naar de mail/ }));
    const nav = within(await screen.findByRole("navigation", { name: "Mappen" }));
    const domainInbox = await nav.findByRole("button", { name: /^Postvak IN jansen-voorbeeld\.nl/ });
    await waitFor(() => expect(domainInbox.textContent).toContain("1 ongelezen"));
    await user.click(domainInbox);
    const list = within(screen.getByRole("region", { name: "Berichten" }));
    await list.findByRole("button", { name: /Familieweekend in mei/ });
    expect(list.queryByRole("button", { name: /Koffie donderdag/ })).toBeNull();

    const send = vi.spyOn(api, "sendMessage");
    await user.click(list.getByRole("button", { name: /Familieweekend in mei/ }));
    await user.click(await screen.findByRole("button", { name: "Beantwoorden" }));
    const from = (await screen.findByLabelText("Van")) as HTMLSelectElement;
    expect(from.selectedOptions[0].textContent).toBe(`Ria Jansen <${FAMILY}>`);
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    await waitFor(() => expect(send).toHaveBeenCalled());
    expect(send.mock.calls[0][0]).toMatchObject({ accountId: "acc-solcon", fromAddress: FAMILY });
  });

  it("takes another address only when the mail was really sent to it", () => {
    const to = [{ email: "Familie@Jansen-Voorbeeld.nl" }];
    expect(replyFromAddress({ to, cc: [] }, [FAMILY])).toBe(FAMILY);
    expect(replyFromAddress({ to: [], cc: [], deliveredTo: [FAMILY] }, [FAMILY])).toBe(FAMILY);
    expect(replyFromAddress({ to: [{ email: "ria.jansen@solcon.nl" }], cc: [] }, [FAMILY])).toBeNull();
  });
});
