import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, hasTauri } from "./client";
import { ALL_INBOXES_ID } from "./commands";
import { toVemError } from "./errors";
import { getMockBackend } from "./mock";
import type { Draft } from "./types";

function draft(patch: Partial<Draft> = {}): Draft {
  return {
    id: null,
    accountId: "acc-solcon",
    to: [{ name: "Anneke", email: "anneke.devries@voorbeeld.nl" }],
    cc: [],
    bcc: [],
    subject: "Proef",
    bodyText: "Hallo",
    attachments: [],
    inReplyTo: null,
    references: [],
    shrinkPhotos: true,
    ...patch,
  };
}

describe("client", () => {
  it("uses the mock without Tauri", async () => {
    expect(hasTauri()).toBe(false);
    expect((await api.listAccounts()).length).toBe(3);
  });

  it("turns anything thrown into a VemError", () => {
    expect(toVemError(new Error("kapot"))).toEqual({ code: "VEM-APP-CMD-01", serverReply: null, detail: "kapot" });
    expect(toVemError("tekst")).toEqual({ code: "VEM-APP-CMD-01", serverReply: null, detail: "tekst" });
    expect(toVemError({ code: "VEM-IMAP-LOGIN-01", serverReply: "NO x" })).toEqual({
      code: "VEM-IMAP-LOGIN-01",
      serverReply: "NO x",
      detail: null,
    });
  });

  it("passes mock errors on as VemError", async () => {
    await expect(api.lookupSettings("iemand@onbekend.nl")).rejects.toMatchObject({ code: "VEM-LOOKUP-FIND-01" });
  });
});

describe("mock: delete and undo", () => {
  it("moves to Verwijderde items and undo puts it back", async () => {
    const before = await api.listMessages("f-s-inbox", 0, 100);
    const target = before[0];
    const token = await api.deleteMessages([target.id]);
    expect(token.count).toBe(1);
    expect((await api.listMessages("f-s-inbox", 0, 100)).map((m) => m.id)).not.toContain(target.id);
    expect((await api.listMessages("f-s-trash", 0, 100)).map((m) => m.id)).toContain(target.id);

    await api.undo(token.token);
    expect((await api.listMessages("f-s-inbox", 0, 100)).map((m) => m.id)).toContain(target.id);
    expect((await api.listMessages("f-s-trash", 0, 100)).map((m) => m.id)).not.toContain(target.id);
  });

  it("removes a message already in Verwijderde items, and undo restores it", async () => {
    const [inTrash] = await api.listMessages("f-s-trash", 0, 10);
    const token = await api.deleteMessages([inTrash.id]);
    expect(getMockBackend().snapshot().messages.find((m) => m.id === inTrash.id)).toBeUndefined();
    await api.undo(token.token);
    expect(getMockBackend().snapshot().messages.find((m) => m.id === inTrash.id)?.folderId).toBe("f-s-trash");
  });

  it("empties Verwijderde items of one account only", async () => {
    await api.emptyTrash("acc-solcon");
    expect(await api.listMessages("f-s-trash", 0, 10)).toHaveLength(0);
    expect((await api.listMessages("f-g-trash", 0, 10)).length).toBeGreaterThan(0);
  });
});

describe("mock: lists and search", () => {
  it("lists Alle postvakken newest first over both mailboxes", async () => {
    const all = await api.listMessages(ALL_INBOXES_ID, 0, 100);
    expect(new Set(all.map((m) => m.accountId))).toEqual(new Set(["acc-solcon", "acc-gmail"]));
    const dates = all.map((m) => m.date);
    expect(dates).toEqual([...dates].sort().reverse());
  });

  it("counts unread per folder", async () => {
    const inbox = (await api.listFolders("acc-solcon")).find((f) => f.id === "f-s-inbox");
    expect(inbox?.unread).toBeGreaterThan(0);
  });

  it("filters search by scope", async () => {
    const all = await api.search("sanne", { kind: "all" }, 100);
    const account = await api.search("sanne", { kind: "account", accountId: "acc-gmail" }, 100);
    const folder = await api.search("sanne", { kind: "folder", folderId: "f-s-klein" }, 100);
    expect(all.length).toBeGreaterThan(account.length);
    expect(account.every((h) => h.message.accountId === "acc-gmail")).toBe(true);
    expect(folder.every((h) => h.message.folderId === "f-s-klein")).toBe(true);
    expect(folder.length).toBeGreaterThan(0);
    expect(all[0].accountName).not.toBe("");
  });

  it("finds nothing for an empty query", async () => {
    expect(await api.search("   ", { kind: "all" }, 10)).toEqual([]);
  });
});

describe("mock: drafts and sending", () => {
  beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }));
  afterEach(() => vi.useRealTimers());

  it("saves a draft into Concepten and loads it back", async () => {
    const id = await api.saveDraft(draft());
    expect((await api.listMessages("f-s-drafts", 0, 10)).map((m) => m.id)).toContain(id);
    expect((await api.loadDraft(id)).subject).toBe("Proef");
  });

  it("puts a sent message in Postvak UIT and cancel gives the draft back", async () => {
    const item = await api.sendMessage(draft({ subject: "Terughalen" }));
    expect(item.state).toBe("waiting");
    expect(await api.listOutbox()).toHaveLength(1);
    const back = await api.cancelSend(item.id);
    expect(back.subject).toBe("Terughalen");
    expect(await api.listOutbox()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(20_000);
    expect((await api.listMessages("f-s-sent", 0, 100)).some((m) => m.subject === "Terughalen")).toBe(false);
  });

  it("moves a sent message to Verzonden items after the wait", async () => {
    const id = await api.saveDraft(draft({ subject: "Echt weg" }));
    await api.sendMessage(draft({ id, subject: "Echt weg" }));
    expect((await api.listMessages("f-s-drafts", 0, 10)).map((m) => m.id)).not.toContain(id);
    await vi.advanceTimersByTimeAsync(11_000);
    expect(await api.listOutbox()).toHaveLength(0);
    expect((await api.listMessages("f-s-sent", 0, 100)).some((m) => m.subject === "Echt weg")).toBe(true);
  });

  it("marks a refused address as failed with the server's reply", async () => {
    await api.sendMessage(draft({ to: [{ email: "weigert@voorbeeld.nl" }] }));
    await vi.advanceTimersByTimeAsync(11_000);
    const [failed] = await api.listOutbox();
    expect(failed.state).toBe("failed");
    expect(failed.error?.code).toBe("VEM-SMTP-CMD-01");
    expect(failed.error?.serverReply).toContain("550 5.1.1");
  });
});

describe("mock: tags and contacts", () => {
  it("toggles a tag and lists its messages across accounts", async () => {
    await api.setMessageTags(["m-26"], "t-familie", true);
    const tagged = await api.listTagMessages("t-familie");
    expect(tagged.map((m) => m.id)).toContain("m-26");
    expect(new Set(tagged.map((m) => m.accountId)).size).toBeGreaterThan(1);
    await api.setMessageTags(["m-26"], "t-familie", false);
    expect((await api.listTagMessages("t-familie")).map((m) => m.id)).not.toContain("m-26");
  });

  it("lists contacts by name and saves a new one", async () => {
    const saved = await api.saveContact({ id: "", name: "Aad Aarts", emails: ["aad@voorbeeld.nl"], phone: null, note: null });
    expect(saved.id).not.toBe("");
    const list = await api.listContacts("");
    expect(list[0].name).toBe("Aad Aarts");
    expect((await api.listContacts("aarts")).map((c) => c.id)).toEqual([saved.id]);
  });
});
