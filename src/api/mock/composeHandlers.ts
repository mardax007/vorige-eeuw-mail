import type { CommandHandlers } from "../commands";
import type { Contact, Draft, MessageDetail, OutboxItem } from "../types";
import { summaryOf } from "./mailHandlers";
import { mockError, newestFirst, roleFolder, type MockStore } from "./state";

type ComposeCommands =
  | "save_draft"
  | "delete_draft"
  | "load_draft"
  | "send_message"
  | "cancel_send"
  | "list_outbox"
  | "pick_attachments"
  | "open_attachment"
  | "save_attachment"
  | "list_contacts"
  | "save_contact"
  | "delete_contact"
  | "contact_messages";

/** A recipient the mock refuses, to show what a send error looks like. */
const REFUSED_ADDRESS = "weigert@voorbeeld.nl";

function messageFromDraft(store: MockStore, id: string, draft: Draft, folderId: string): MessageDetail {
  const account = store.get().accounts.find((a) => a.id === draft.accountId);
  return {
    id,
    accountId: draft.accountId,
    folderId,
    from: { name: account?.displayName ?? null, email: account?.email ?? "" },
    to: draft.to,
    subject: draft.subject,
    date: new Date().toISOString(),
    preview: draft.bodyText.replace(/\s+/g, " ").slice(0, 120),
    unread: false,
    important: false,
    hasAttachments: draft.attachments.length > 0,
    tagIds: [],
    cc: draft.cc,
    replyTo: [],
    html: null,
    text: draft.bodyText,
    hasRemoteImages: false,
    remoteImagesShown: false,
    attachments: draft.attachments.map((path, i) => ({
      id: `${id}-a${i}`,
      name: path.split(/[\\/]/).pop() ?? path,
      size: 100_000,
      mime: "application/octet-stream",
    })),
    warnings: [],
    messageIdHeader: `<${id}@voorbeeld.nl>`,
    references: draft.references,
  };
}

function removeDraft(store: MockStore, draftId: string): void {
  store.set((s) => {
    const { [draftId]: _gone, ...drafts } = s.drafts;
    return { ...s, drafts, messages: s.messages.filter((m) => m.id !== draftId) };
  });
}

function emitOutbox(store: MockStore): void {
  store.events.emit("outbox:changed", store.get().outbox.map((o) => o.item));
}

function finishSend(store: MockStore, outboxId: string): void {
  const entry = store.get().outbox.find((o) => o.item.id === outboxId);
  if (!entry) return;
  const refused = [...entry.draft.to, ...entry.draft.cc].some((a) => a.email === REFUSED_ADDRESS);
  if (refused) {
    const error = mockError(
      "VEM-SMTP-CMD-01",
      `550 5.1.1 <${REFUSED_ADDRESS}>: Recipient address rejected: User unknown in virtual mailbox table`,
    );
    store.set((s) => ({
      ...s,
      outbox: s.outbox.map((o) =>
        o.item.id === outboxId ? { ...o, item: { ...o.item, state: "failed", error } } : o,
      ),
    }));
    emitOutbox(store);
    return;
  }
  const sent = roleFolder(store.get(), entry.draft.accountId, "sent");
  const message = messageFromDraft(store, store.nextId("m"), entry.draft, sent?.id ?? "");
  store.set((s) => ({
    ...s,
    outbox: s.outbox.filter((o) => o.item.id !== outboxId),
    messages: sent ? [...s.messages, message] : s.messages,
  }));
  emitOutbox(store);
  store.events.emit("mail:changed", { accountId: entry.draft.accountId });
}

const timers = new Map<string, ReturnType<typeof setTimeout>>();

function contactMatches(c: Contact, q: string): boolean {
  return [c.name, ...c.emails].some((v) => v.toLowerCase().includes(q));
}

export function createComposeHandlers(store: MockStore): Pick<CommandHandlers, ComposeCommands> {
  return {
    save_draft: ({ draft }) => {
      const id = draft.id ?? store.nextId("d");
      const saved = { ...draft, id };
      const folder = roleFolder(store.get(), draft.accountId, "drafts");
      const message = messageFromDraft(store, id, saved, folder?.id ?? "");
      store.set((s) => ({
        ...s,
        drafts: { ...s.drafts, [id]: saved },
        messages: [...s.messages.filter((m) => m.id !== id), message],
      }));
      return id;
    },
    delete_draft: ({ draftId }) => removeDraft(store, draftId),
    load_draft: ({ messageId }) => {
      const draft = store.get().drafts[messageId];
      if (!draft) throw mockError("VEM-STORE-READ-01", undefined, `draft ${messageId}`);
      return draft;
    },
    send_message: ({ draft }) => {
      if (draft.id) removeDraft(store, draft.id);
      const seconds = store.get().settings.undoSendSeconds;
      const item: OutboxItem = {
        id: store.nextId("o"),
        subject: draft.subject,
        sendAt: new Date(Date.now() + seconds * 1000).toISOString(),
        state: "waiting",
        error: null,
      };
      store.set((s) => ({ ...s, outbox: [...s.outbox, { item, draft: { ...draft, id: null } }] }));
      timers.set(item.id, store.later(seconds * 1000, () => finishSend(store, item.id)));
      emitOutbox(store);
      return item;
    },
    cancel_send: ({ outboxId }) => {
      const entry = store.get().outbox.find((o) => o.item.id === outboxId);
      if (!entry || entry.item.state === "sending") {
        throw mockError("VEM-APP-CMD-01", undefined, "already sent");
      }
      const timer = timers.get(outboxId);
      if (timer !== undefined) store.cancel(timer);
      timers.delete(outboxId);
      store.set((s) => ({ ...s, outbox: s.outbox.filter((o) => o.item.id !== outboxId) }));
      emitOutbox(store);
      return entry.draft;
    },
    list_outbox: () => store.get().outbox.map((o) => o.item),
    pick_attachments: () => ["C:\\Users\\Ria\\Pictures\\Kleinkinderen.jpg"],
    open_attachment: () => undefined,
    save_attachment: () => true,
    list_contacts: ({ query }) => {
      const q = query.trim().toLowerCase();
      return store
        .get()
        .contacts.filter((c) => q === "" || contactMatches(c, q))
        .sort((a, b) => a.name.localeCompare(b.name, "nl"));
    },
    save_contact: ({ contact }) => {
      const saved = { ...contact, id: contact.id || store.nextId("c") };
      store.set((s) => ({
        ...s,
        contacts: [...s.contacts.filter((c) => c.id !== saved.id), saved],
      }));
      return saved;
    },
    delete_contact: ({ contactId }) =>
      store.set((s) => ({ ...s, contacts: s.contacts.filter((c) => c.id !== contactId) })),
    contact_messages: ({ contactId }) => {
      const s = store.get();
      const contact = s.contacts.find((c) => c.id === contactId);
      const emails = new Set(contact?.emails.map((e) => e.toLowerCase()) ?? []);
      const related = s.messages.filter(
        (m) =>
          emails.has(m.from.email.toLowerCase()) ||
          m.to.some((a) => emails.has(a.email.toLowerCase())),
      );
      return newestFirst(related).slice(0, 20).map(summaryOf);
    },
  };
}

export function clearComposeTimers(): void {
  timers.clear();
}
