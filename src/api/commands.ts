// Every command the screens call, with its arguments and result.
// Command names are the Rust names (snake_case); argument keys are camelCase.

import type {
  Account,
  AppInfo,
  Contact,
  ContactsImported,
  Draft,
  Folder,
  ImportKind,
  ImportProgress,
  ImportSource,
  ImportTarget,
  MessageDetail,
  MessageSummary,
  OriginalCopy,
  OutboxItem,
  PairInfo,
  SearchHit,
  SearchScope,
  ServerSettings,
  Settings,
  ShellTexts,
  SignIn,
  Tag,
  UndoToken,
} from "./types";

/** The folder id of the unified "Alle postvakken" view (C7). */
export const ALL_INBOXES_ID = "all-inboxes";

type NoArgs = undefined;

export interface CommandMap {
  app_info: { args: NoArgs; result: AppInfo };
  set_shell_texts: { args: { texts: ShellTexts }; result: void };
  list_accounts: { args: NoArgs; result: Account[] };
  lookup_settings: { args: { email: string }; result: ServerSettings };
  add_account: {
    args: { displayName: string; email: string; password: string; settings: ServerSettings };
    result: Account;
  };
  oauth_available: { args: { signIn: SignIn }; result: boolean };
  add_account_oauth: {
    args: { displayName: string; email: string; settings: ServerSettings; donePage: string };
    result: Account;
  };
  /** Tries the new password on the mailbox first; saves nothing when it does not work. */
  update_password: { args: { accountId: string; password: string }; result: void };
  /** Opens the browser for Google or Microsoft again. */
  reauth_account: { args: { accountId: string; donePage: string }; result: void };
  /** Stops a waiting browser sign-in; that call then fails with VEM-AUTH-LOGIN-01, detail "stopped". */
  cancel_sign_in: { args: NoArgs; result: void };
  remove_account: { args: { accountId: string }; result: void };
  set_signature: { args: { accountId: string; signature: string }; result: void };

  list_folders: { args: { accountId: string | null }; result: Folder[] };
  list_messages: {
    args: { folderId: string; offset: number; limit: number };
    result: MessageSummary[];
  };
  get_message: { args: { messageId: string; showRemoteImages: boolean }; result: MessageDetail };

  set_read: { args: { messageIds: string[]; read: boolean }; result: void };
  set_important: { args: { messageIds: string[]; important: boolean }; result: void };
  delete_messages: { args: { messageIds: string[] }; result: UndoToken };
  move_messages: { args: { messageIds: string[]; folderId: string }; result: UndoToken };
  undo: { args: { token: string }; result: void };
  empty_trash: { args: { accountId: string }; result: void };

  search: { args: { query: string; scope: SearchScope; limit: number }; result: SearchHit[] };

  list_tags: { args: NoArgs; result: Tag[] };
  create_tag: { args: { name: string; color: string }; result: Tag };
  delete_tag: { args: { tagId: string }; result: void };
  set_message_tags: { args: { messageIds: string[]; tagId: string; on: boolean }; result: void };
  list_tag_messages: { args: { tagId: string }; result: MessageSummary[] };

  list_contacts: { args: { query: string }; result: Contact[] };
  save_contact: { args: { contact: Contact }; result: Contact };
  delete_contact: { args: { contactId: string }; result: void };
  contact_messages: { args: { contactId: string }; result: MessageSummary[] };

  save_draft: { args: { draft: Draft }; result: string };
  delete_draft: { args: { draftId: string }; result: void };
  load_draft: { args: { messageId: string }; result: Draft };
  send_message: { args: { draft: Draft }; result: OutboxItem };
  cancel_send: { args: { outboxId: string }; result: Draft };
  list_outbox: { args: NoArgs; result: OutboxItem[] };
  pick_attachments: { args: NoArgs; result: string[] };
  open_attachment: { args: { messageId: string; attachmentId: string }; result: void };
  save_attachment: { args: { messageId: string; attachmentId: string }; result: void };

  sync_now: { args: { accountId: string | null }; result: void };
  get_settings: { args: NoArgs; result: Settings };
  save_settings: { args: { settings: Settings }; result: void };

  find_import_sources: { args: NoArgs; result: ImportSource[] };
  pick_import_file: { args: { kind: ImportKind }; result: ImportSource | null };
  /** Reads vCard files (a folder of them, or one file) into the address book; null when cancelled. */
  import_contacts_file: { args: { folder: boolean }; result: ContactsImported | null };
  start_import: { args: { source: ImportSource; target: ImportTarget; password: string | null }; result: string };
  /** Copies the mail of an added mailbox into the target; the source mailbox is only read. */
  import_from_account: { args: { sourceAccountId: string; target: ImportTarget }; result: string };
  /** The running import's latest progress, or the last finished one. */
  import_status: { args: NoArgs; result: ImportProgress | null };
  list_originals: { args: NoArgs; result: OriginalCopy[] };
  delete_original: { args: { id: string }; result: void };
  export_folder: { args: { folderId: string }; result: string | null };

  send_wish: { args: { text: string }; result: void };
  help_report: { args: NoArgs; result: string };
  pair_phone: { args: NoArgs; result: PairInfo };
  phone_paired: { args: NoArgs; result: boolean };
  unpair_phone: { args: NoArgs; result: void };
  quit_app: { args: NoArgs; result: void };
}

export type CommandName = keyof CommandMap;
export type CommandArgs<K extends CommandName> = CommandMap[K]["args"];
export type CommandResult<K extends CommandName> = CommandMap[K]["result"];

/** A full implementation of every command (the mock backend implements this). */
export type CommandHandlers = {
  [K in CommandName]: (
    args: CommandArgs<K>,
  ) => CommandResult<K> | Promise<CommandResult<K>>;
};
