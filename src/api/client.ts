// Typed wrappers around the Rust core's commands and events.
// Without Tauri (a plain browser, tests) the in-memory mock backend answers instead.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CommandArgs, CommandName, CommandResult } from "./commands";
import { toVemError } from "./errors";
import type { CoreEvents, Draft, ImportKind, ImportSource, ImportTarget } from "./types";
import type { Contact, SearchScope, ServerSettings, Settings, ShellTexts, SignIn } from "./types";

export type Unlisten = () => void;
export type CoreEventName = keyof CoreEvents;

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function hasTauri(): boolean {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function mock() {
  const mod = await import("./mock");
  return mod.getMockBackend();
}

async function call<K extends CommandName>(
  cmd: K,
  args: CommandArgs<K>,
): Promise<CommandResult<K>> {
  try {
    if (hasTauri()) {
      return await invoke<CommandResult<K>>(cmd, args ?? undefined);
    }
    return await (await mock()).call(cmd, args);
  } catch (e: unknown) {
    throw toVemError(e);
  }
}

/** Listens to one core event. Returns a function that stops listening. */
export async function onEvent<E extends CoreEventName>(
  name: E,
  handler: (payload: CoreEvents[E]) => void,
): Promise<Unlisten> {
  if (hasTauri()) {
    return listen<CoreEvents[E]>(name, (event) => handler(event.payload));
  }
  return (await mock()).events.on(name, handler);
}

/** Every command, one function each. Components call these through this object. */
export const api = {
  appInfo: () => call("app_info", undefined),
  updateOffer: () => call("update_offer", undefined),
  installUpdate: () => call("install_update", undefined),
  setShellTexts: (texts: ShellTexts) => call("set_shell_texts", { texts }),
  listAccounts: () => call("list_accounts", undefined),
  lookupSettings: (email: string) => call("lookup_settings", { email }),
  addAccount: (displayName: string, email: string, password: string, settings: ServerSettings) =>
    call("add_account", { displayName, email, password, settings }),
  oauthAvailable: (signIn: SignIn) => call("oauth_available", { signIn }),
  addAccountOAuth: (displayName: string, email: string, settings: ServerSettings, donePage: string) =>
    call("add_account_oauth", { displayName, email, settings, donePage }),
  updatePassword: (accountId: string, password: string) =>
    call("update_password", { accountId, password }),
  reauthAccount: (accountId: string, donePage: string) =>
    call("reauth_account", { accountId, donePage }),
  cancelSignIn: () => call("cancel_sign_in", undefined),
  removeAccount: (accountId: string) => call("remove_account", { accountId }),
  setSignature: (accountId: string, signature: string) =>
    call("set_signature", { accountId, signature }),

  listFolders: (accountId: string | null) => call("list_folders", { accountId }),
  listMessages: (folderId: string, offset: number, limit: number) =>
    call("list_messages", { folderId, offset, limit }),
  getMessage: (messageId: string, showRemoteImages: boolean) =>
    call("get_message", { messageId, showRemoteImages }),

  setRead: (messageIds: string[], read: boolean) => call("set_read", { messageIds, read }),
  setImportant: (messageIds: string[], important: boolean) =>
    call("set_important", { messageIds, important }),
  deleteMessages: (messageIds: string[]) => call("delete_messages", { messageIds }),
  moveMessages: (messageIds: string[], folderId: string) =>
    call("move_messages", { messageIds, folderId }),
  undo: (token: string) => call("undo", { token }),
  emptyTrash: (accountId: string) => call("empty_trash", { accountId }),

  search: (query: string, scope: SearchScope, limit: number) =>
    call("search", { query, scope, limit }),

  listTags: () => call("list_tags", undefined),
  createTag: (name: string, color: string) => call("create_tag", { name, color }),
  deleteTag: (tagId: string) => call("delete_tag", { tagId }),
  setMessageTags: (messageIds: string[], tagId: string, on: boolean) =>
    call("set_message_tags", { messageIds, tagId, on }),
  listTagMessages: (tagId: string) => call("list_tag_messages", { tagId }),

  listContacts: (query: string) => call("list_contacts", { query }),
  saveContact: (contact: Contact) => call("save_contact", { contact }),
  deleteContact: (contactId: string) => call("delete_contact", { contactId }),
  contactMessages: (contactId: string) => call("contact_messages", { contactId }),

  saveDraft: (draft: Draft) => call("save_draft", { draft }),
  deleteDraft: (draftId: string) => call("delete_draft", { draftId }),
  loadDraft: (messageId: string) => call("load_draft", { messageId }),
  sendMessage: (draft: Draft) => call("send_message", { draft }),
  cancelSend: (outboxId: string) => call("cancel_send", { outboxId }),
  listOutbox: () => call("list_outbox", undefined),
  pickAttachments: () => call("pick_attachments", undefined),
  openAttachment: (messageId: string, attachmentId: string) =>
    call("open_attachment", { messageId, attachmentId }),
  saveAttachment: (messageId: string, attachmentId: string) =>
    call("save_attachment", { messageId, attachmentId }),

  syncNow: (accountId: string | null) => call("sync_now", { accountId }),
  getSettings: () => call("get_settings", undefined),
  saveSettings: (settings: Settings) => call("save_settings", { settings }),

  findImportSources: () => call("find_import_sources", undefined),
  pickImportFile: (kind: ImportKind) => call("pick_import_file", { kind }),
  importContactsFile: (folder: boolean) => call("import_contacts_file", { folder }),
  startImport: (source: ImportSource, target: ImportTarget, password: string | null = null, rescue = false) =>
    call("start_import", { source, target, password, rescue }),
  rescueAvailable: () => call("rescue_available", undefined),
  importFromAccount: (sourceAccountId: string, target: ImportTarget) =>
    call("import_from_account", { sourceAccountId, target }),
  importStatus: () => call("import_status", undefined),
  listOriginals: () => call("list_originals", undefined),
  deleteOriginal: (id: string) => call("delete_original", { id }),
  exportFolder: (folderId: string) => call("export_folder", { folderId }),

  sendWish: (text: string) => call("send_wish", { text }),
  helpReport: () => call("help_report", undefined),
  pairPhone: () => call("pair_phone", undefined),
  phonePaired: () => call("phone_paired", undefined),
  pairWithCode: (text: string) => call("pair_with_code", { text }),
  unpairPhone: () => call("unpair_phone", undefined),
  quitApp: () => call("quit_app", undefined),
};

export type Api = typeof api;
