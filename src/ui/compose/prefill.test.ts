import { describe, expect, it } from "vitest";
import type { Draft, MessageDetail } from "../../api/types";
import { parseAddressList, replaceLastToken } from "./address";
import { plainBody, prefillDraft, withPrefix } from "./prefill";

const base: Draft = {
  id: null, accountId: "acc-solcon", to: [], cc: [], bcc: [], subject: "", bodyText: "",
  attachments: [], inReplyTo: null, references: [], shrinkPhotos: true,
};

const message: MessageDetail = {
  id: "m-1", accountId: "acc-solcon", folderId: "f-s-inbox",
  from: { name: "Kees", email: "kees@voorbeeld.nl" },
  to: [{ name: "Ria", email: "ria.jansen@solcon.nl" }, { name: "Bep", email: "bep@voorbeeld.nl" }],
  cc: [{ email: "wim@voorbeeld.nl" }], replyTo: [],
  subject: "Verjaardag", date: "2026-11-10T10:00:00.000Z", preview: "", unread: false, important: false,
  hasAttachments: false, tagIds: [], html: null, text: "Regel een\nRegel twee", hasRemoteImages: false,
  remoteImagesShown: false, attachments: [], warnings: [], messageIdHeader: "<abc@voorbeeld.nl>", references: ["<eerder@voorbeeld.nl>"],
};

describe("prefill", () => {
  it("answers the sender with Re: and quoted text and threading headers", () => {
    const d = prefillDraft(base, message, "reply", "ria.jansen@solcon.nl", "Groet, Ria");
    expect(d.to.map((a) => a.email)).toEqual(["kees@voorbeeld.nl"]);
    expect(d.subject).toBe("Re: Verjaardag");
    expect(d.bodyText).toContain("> Regel een\n> Regel twee");
    expect(d.bodyText).toContain("Groet, Ria");
    expect(d.inReplyTo).toBe("<abc@voorbeeld.nl>");
    expect(d.references).toEqual(["<eerder@voorbeeld.nl>", "<abc@voorbeeld.nl>"]);
  });

  it("answers everyone except the user", () => {
    const d = prefillDraft(base, message, "replyAll", "ria.jansen@solcon.nl", null);
    expect(d.to.map((a) => a.email)).toEqual(["kees@voorbeeld.nl", "bep@voorbeeld.nl"]);
    expect(d.cc.map((a) => a.email)).toEqual(["wim@voorbeeld.nl"]);
  });

  it("forwards with Fw: and no recipients", () => {
    const d = prefillDraft(base, message, "forward", "ria.jansen@solcon.nl", null);
    expect(d.subject).toBe("Fw: Verjaardag");
    expect(d.to).toEqual([]);
    expect(d.inReplyTo).toBeNull();
    expect(d.bodyText).toContain("Doorgestuurd bericht");
  });

  it("does not stack prefixes", () => {
    expect(withPrefix("Re: Koffie", "reply")).toBe("Re: Koffie");
    expect(withPrefix("RE: Koffie", "reply")).toBe("RE: Koffie");
    expect(withPrefix("Fw: Koffie", "forward")).toBe("Fw: Koffie");
  });

  it("reads plain text out of HTML-only mail", () => {
    expect(plainBody({ text: null, html: "<p>Hallo <b>daar</b></p>" })).toBe("Hallo daar");
  });
});

describe("address list", () => {
  it("reads names, bare addresses and finds wrong ones", () => {
    const parsed = parseAddressList('Jan de Vries <jan@voorbeeld.nl>, piet@voorbeeld.nl; "Bakker, Sanne" <sanne@voorbeeld.nl>, fout');
    expect(parsed.addresses).toEqual([
      { name: "Jan de Vries", email: "jan@voorbeeld.nl" },
      { name: null, email: "piet@voorbeeld.nl" },
      { name: "Bakker, Sanne", email: "sanne@voorbeeld.nl" },
    ]);
    expect(parsed.invalid).toEqual(["fout"]);
  });

  it("replaces what is being typed with the chosen contact", () => {
    expect(replaceLastToken("jan@voorbeeld.nl, ann", { name: "Anneke", email: "anneke@voorbeeld.nl" })).toBe(
      "jan@voorbeeld.nl, Anneke <anneke@voorbeeld.nl>, ",
    );
  });
});
