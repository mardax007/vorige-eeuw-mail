import { ALL_INBOXES_ID, type CommandHandlers } from "../commands";
import type { Folder, MessageDetail, MessageSummary, SearchHit, SearchScope } from "../types";
import {
  folderOf,
  mockError,
  newestFirst,
  roleFolder,
  type MockState,
  type MockStore,
} from "./state";

type MailCommands =
  | "list_folders"
  | "list_messages"
  | "get_message"
  | "set_read"
  | "set_important"
  | "delete_messages"
  | "move_messages"
  | "undo"
  | "empty_trash"
  | "search"
  | "list_tags"
  | "create_tag"
  | "delete_tag"
  | "set_message_tags"
  | "list_tag_messages"
  | "list_domain_inboxes"
  | "list_domain_messages";

/** Postvak IN of a mailbox, only mail with an address at the domain in To or Cc (decision 32). */
function domainMessages(s: MockState, accountId: string, domain: string): MessageDetail[] {
  const inbox = roleFolder(s, accountId, "inbox");
  const at = (e: string) => e.toLowerCase().endsWith(`@${domain}`);
  return s.messages.filter((m) => m.folderId === inbox?.id && [...m.to, ...m.cc].some((a) => at(a.email)));
}

export function summaryOf(m: MessageDetail): MessageSummary {
  return {
    id: m.id,
    accountId: m.accountId,
    folderId: m.folderId,
    from: m.from,
    to: m.to,
    subject: m.subject,
    date: m.date,
    preview: m.preview,
    unread: m.unread,
    important: m.important,
    hasAttachments: m.hasAttachments,
    tagIds: m.tagIds,
  };
}

function inboxIds(state: MockState): Set<string> {
  const imap = new Set(state.accounts.filter((a) => a.kind === "imap").map((a) => a.id));
  return new Set(
    state.folders.filter((f) => f.role === "inbox" && imap.has(f.accountId)).map((f) => f.id),
  );
}

function matchesScope(state: MockState, m: MessageDetail, scope: SearchScope): boolean {
  if (scope.kind === "all") return true;
  if (scope.kind === "account") return m.accountId === scope.accountId;
  if (scope.folderId === ALL_INBOXES_ID) return inboxIds(state).has(m.folderId);
  return m.folderId === scope.folderId;
}

function haystack(m: MessageDetail): string {
  return [m.subject, m.from.name ?? "", m.from.email, m.preview, m.text ?? ""]
    .join(" ")
    .toLowerCase();
}

function patchMessages(
  store: MockStore,
  ids: readonly string[],
  patch: (m: MessageDetail) => MessageDetail,
): void {
  const wanted = new Set(ids);
  store.set((s) => ({
    ...s,
    messages: s.messages.map((m) => (wanted.has(m.id) ? patch(m) : m)),
  }));
}

function notifyChanged(store: MockStore, accountIds: readonly string[]): void {
  new Set(accountIds).forEach((accountId) => store.events.emit("mail:changed", { accountId }));
}

/** Moves messages and remembers how they were, so undo can put them back. */
function relocate(
  store: MockStore,
  ids: readonly string[],
  target: (m: MessageDetail, s: MockState) => string | null,
): { token: string; count: number } {
  const state = store.get();
  const wanted = new Set(ids);
  const before = state.messages.filter((m) => wanted.has(m.id));
  const token = store.nextId("undo");
  store.set((s) => ({
    ...s,
    undo: { ...s.undo, [token]: { before } },
    messages: s.messages.flatMap((m) => {
      if (!wanted.has(m.id)) return [m];
      const folderId = target(m, s);
      return folderId === null ? [] : [{ ...m, folderId }];
    }),
  }));
  notifyChanged(store, before.map((m) => m.accountId));
  return { token, count: before.length };
}

export function createMailHandlers(store: MockStore): Pick<CommandHandlers, MailCommands> {
  return {
    list_folders: ({ accountId }) => {
      const s = store.get();
      return s.folders
        .filter((f) => accountId === null || f.accountId === accountId)
        .map((f): Folder => {
          const inFolder = s.messages.filter((m) => m.folderId === f.id);
          return { ...f, total: inFolder.length, unread: inFolder.filter((m) => m.unread).length };
        });
    },
    list_messages: ({ folderId, offset, limit }) => {
      const s = store.get();
      const inboxes = inboxIds(s);
      const list = s.messages.filter((m) =>
        folderId === ALL_INBOXES_ID ? inboxes.has(m.folderId) : m.folderId === folderId,
      );
      return newestFirst(list).slice(offset, offset + limit).map(summaryOf);
    },
    get_message: ({ messageId, showRemoteImages }) => {
      const m = store.get().messages.find((x) => x.id === messageId);
      if (!m) throw mockError("VEM-STORE-READ-01", undefined, `message ${messageId}`);
      return { ...m, remoteImagesShown: showRemoteImages && m.hasRemoteImages };
    },
    set_read: ({ messageIds, read }) => {
      patchMessages(store, messageIds, (m) => ({ ...m, unread: !read }));
      notifyChanged(store, store.get().messages.filter((m) => messageIds.includes(m.id)).map((m) => m.accountId));
    },
    set_important: ({ messageIds, important }) =>
      patchMessages(store, messageIds, (m) => ({ ...m, important })),
    delete_messages: ({ messageIds }) =>
      relocate(store, messageIds, (m, s) => {
        const trash = roleFolder(s, m.accountId, "trash");
        if (!trash || m.folderId === trash.id) return null;
        return trash.id;
      }),
    move_messages: ({ messageIds, folderId }) => {
      if (!folderOf(store.get(), folderId)) throw mockError("VEM-IMAP-CMD-01", "NO [TRYCREATE] Mailbox doesn't exist");
      return relocate(store, messageIds, () => folderId);
    },
    undo: ({ token }) => {
      const entry = store.get().undo[token];
      if (!entry) return;
      const ids = new Set(entry.before.map((m) => m.id));
      store.set((s) => {
        const { [token]: _used, ...rest } = s.undo;
        return { ...s, undo: rest, messages: [...s.messages.filter((m) => !ids.has(m.id)), ...entry.before] };
      });
      notifyChanged(store, entry.before.map((m) => m.accountId));
    },
    empty_trash: ({ accountId }) => {
      const trash = roleFolder(store.get(), accountId, "trash");
      store.set((s) => ({ ...s, messages: s.messages.filter((m) => m.folderId !== trash?.id) }));
      notifyChanged(store, [accountId]);
    },
    search: ({ query, scope, limit }) => {
      const words = query.toLowerCase().split(/\s+/).filter(Boolean);
      if (words.length === 0) return [];
      const s = store.get();
      const found = s.messages.filter(
        (m) => matchesScope(s, m, scope) && words.every((w) => haystack(m).includes(w)),
      );
      return newestFirst(found)
        .slice(0, limit)
        .map((m): SearchHit => ({
          message: summaryOf(m),
          accountName: s.accounts.find((a) => a.id === m.accountId)?.displayName ?? "",
          folderName: folderOf(s, m.folderId)?.name ?? "",
        }));
    },
    list_tags: () => store.get().tags,
    create_tag: ({ name, color }) => {
      const tag = { id: store.nextId("t"), name, color };
      store.set((s) => ({ ...s, tags: [...s.tags, tag] }));
      return tag;
    },
    delete_tag: ({ tagId }) =>
      store.set((s) => ({
        ...s,
        tags: s.tags.filter((t) => t.id !== tagId),
        messages: s.messages.map((m) => ({ ...m, tagIds: m.tagIds.filter((t) => t !== tagId) })),
      })),
    set_message_tags: ({ messageIds, tagId, on }) =>
      patchMessages(store, messageIds, (m) => {
        const rest = m.tagIds.filter((t) => t !== tagId);
        return { ...m, tagIds: on ? [...rest, tagId] : rest };
      }),
    list_tag_messages: ({ tagId }) =>
      newestFirst(store.get().messages.filter((m) => m.tagIds.includes(tagId))).map(summaryOf),
    list_domain_inboxes: () => {
      const s = store.get();
      return s.accounts.flatMap((a) => {
        const main = a.email.split("@")[1]?.toLowerCase();
        const domains = [...new Set(a.addresses.map((x) => x.split("@")[1]).filter((d) => d && d !== main))];
        return domains.map((domain) => ({
          accountId: a.id,
          domain,
          unread: domainMessages(s, a.id, domain).filter((m) => m.unread).length,
        }));
      });
    },
    list_domain_messages: ({ accountId, domain, offset, limit }) =>
      newestFirst(domainMessages(store.get(), accountId, domain)).slice(offset, offset + limit).map(summaryOf),
  };
}
