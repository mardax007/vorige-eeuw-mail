import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api/client";
import type { Draft } from "../../api/types";
import { AppProvider } from "../state/AppProvider";
import { ComposeView } from "./ComposeView";
import { editorText, hasFormatting, textToHtml } from "./richText";

const draft: Draft = {
  id: null, accountId: "acc-solcon", to: [], cc: [], bcc: [], subject: "", bodyText: "",
  attachments: [], inReplyTo: null, references: [], shrinkPhotos: true,
};

function renderCompose(initial: Draft = draft) {
  render(
    <AppProvider>
      <ComposeView initial={initial} autosaveMs={60_000} onClose={vi.fn()} />
    </AppProvider>,
  );
  return userEvent.setup();
}

async function sendAndGetDraft(user: ReturnType<typeof userEvent.setup>): Promise<Draft> {
  const send = vi.spyOn(api, "sendMessage");
  await user.click(screen.getByRole("button", { name: "Versturen" }));
  await waitFor(() => expect(send).toHaveBeenCalled());
  return send.mock.calls[0][0];
}

afterEach(() => vi.restoreAllMocks());

describe("Write: BCC (decision 33)", () => {
  it("keeps BCC behind a button, explains it, and sends it apart from Aan", async () => {
    const user = renderCompose();
    expect(screen.queryByLabelText("BCC")).toBeNull();
    await user.click(screen.getByRole("button", { name: "BCC toevoegen" }));
    const bcc = screen.getByLabelText("BCC");
    expect(bcc.getAttribute("aria-describedby")).toBe("compose-bcc-hint");
    expect(screen.getByText(/zien niet dat deze mensen/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "CC toevoegen" })).toBeTruthy();

    await user.type(bcc, "geen-adres");
    await user.click(screen.getByRole("button", { name: "Versturen" }));
    expect(screen.getByRole("alert").textContent).toBe("Dit adres klopt niet: geen-adres");
    expect(document.activeElement).toBe(bcc);

    await user.clear(bcc);
    await user.type(bcc, "oma@voorbeeld.nl");
    const sent = await sendAndGetDraft(user);
    expect(sent.to).toEqual([]);
    expect(sent.bcc).toEqual([{ name: null, email: "oma@voorbeeld.nl" }]);
  });
});

describe("Write: the text (decision 33)", () => {
  it("checks Dutch spelling in the subject and the text", () => {
    renderCompose();
    for (const field of [screen.getByLabelText("Onderwerp"), screen.getByLabelText("Bericht")]) {
      expect(field.getAttribute("spellcheck")).toBe("true");
      expect(field.getAttribute("lang")).toBe("nl");
    }
  });

  it("sends plain text as long as nothing is formatted", async () => {
    const user = renderCompose({ ...draft, to: [{ name: null, email: "oma@voorbeeld.nl" }] });
    await user.type(screen.getByLabelText("Bericht"), "Lieve oma");
    const sent = await sendAndGetDraft(user);
    expect(sent.bodyText).toBe("Lieve oma");
    expect(sent.bodyHtml).toBeNull();
  });

  it("makes text bold with the Vet button and sends it as formatted text", async () => {
    const user = renderCompose({ ...draft, to: [{ name: null, email: "oma@voorbeeld.nl" }] });
    const editor = screen.getByLabelText("Bericht");
    await user.type(editor, "Hoera");
    let bold = false;
    // jsdom has no editing commands; this one does what the WebView does with "bold".
    Object.assign(document, {
      execCommand: vi.fn((command: string) => {
        if (command !== "bold") return false;
        editor.innerHTML = `<b>${editor.innerHTML}</b>`;
        bold = true;
        return true;
      }),
      queryCommandState: vi.fn((command: string) => command === "bold" && bold),
    });
    try {
      await user.click(screen.getByRole("button", { name: "Vet" }));
      expect(document.execCommand).toHaveBeenCalledWith("bold", false, undefined);
      expect(screen.getByRole("button", { name: "Vet" }).getAttribute("aria-pressed")).toBe("true");
      const sent = await sendAndGetDraft(user);
      expect(sent.bodyHtml).toBe("<b>Hoera</b>");
      expect(sent.bodyText).toBe("Hoera");
    } finally {
      Reflect.deleteProperty(document, "execCommand");
      Reflect.deleteProperty(document, "queryCommandState");
    }
  });

  it("places a photo in the text", async () => {
    const user = renderCompose({ ...draft, to: [{ name: null, email: "oma@voorbeeld.nl" }] });
    await user.type(screen.getByLabelText("Bericht"), "Kijk eens");
    await user.click(screen.getByRole("button", { name: "Foto in de tekst" }));
    const img = await waitFor(() => {
      const found = screen.getByLabelText("Bericht").querySelector("img");
      expect(found).not.toBeNull();
      return found as HTMLImageElement;
    });
    expect(img.getAttribute("src")).toMatch(/^data:image\/png;base64,/);
    const sent = await sendAndGetDraft(user);
    expect(sent.bodyHtml).toContain("<img");
    // Where the photo lands depends on the cursor, which jsdom does not move while typing.
    expect(sent.bodyText).toContain("Kijk eens");
    expect(sent.bodyText).toContain("[foto]");
  });

  it("says so when a photo cannot go in the text", async () => {
    vi.spyOn(api, "pickTextPhoto").mockResolvedValue({ dataUrl: null });
    const user = renderCompose();
    await user.click(screen.getByRole("button", { name: "Foto in de tekst" }));
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Deze foto kan niet in de tekst. Voeg hem toe met Bijlage toevoegen.",
    );
    expect(screen.getByLabelText("Bericht").querySelector("img")).toBeNull();
  });

  it("pastes text without its formatting", () => {
    renderCompose();
    const editor = screen.getByLabelText("Bericht");
    editor.focus();
    const range = document.createRange();
    range.selectNodeContents(editor);
    document.getSelection()?.addRange(range);
    fireEvent.paste(editor, { clipboardData: { getData: () => "<b>vet</b> <script>x()</script>" } });
    expect(editor.querySelector("b, script")).toBeNull();
    expect(editor.textContent).toBe("<b>vet</b> <script>x()</script>");
  });
});

describe("richText", () => {
  it("turns a signature and a quoted reply into lines and back", () => {
    const text = "\nGroet, Ria\n\nOp 1 mei schreef Jan:\n> Kom je <zaterdag>?";
    const div = document.createElement("div");
    div.innerHTML = textToHtml(text);
    expect(div.querySelectorAll("div")).toHaveLength(5);
    expect(div.innerHTML).toContain("&gt; Kom je &lt;zaterdag&gt;?");
    expect(editorText(div)).toBe(text);
    expect(hasFormatting(div)).toBe(false);
  });

  it("writes a list with dashes and knows it is formatted", () => {
    const div = document.createElement("div");
    div.innerHTML = "Boodschappen:<ul><li>brood</li><li>kaas</li></ul>Tot straks";
    expect(editorText(div)).toBe("Boodschappen:\n- brood\n- kaas\nTot straks");
    expect(hasFormatting(div)).toBe(true);
  });
});
