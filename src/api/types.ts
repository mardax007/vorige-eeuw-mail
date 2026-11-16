// Contract between the screens and the Rust core (src-tauri commands).
// Field names are camelCase on both sides (serde rename_all = "camelCase").
// Change this file and src-tauri/src/commands.rs together.

export type FolderRole =
  | "inbox"
  | "sent"
  | "trash"
  | "drafts"
  | "junk"
  | "archive"
  | "outbox";

export interface Addr {
  name?: string | null;
  email: string;
}

export interface Quota {
  usedKb: number;
  limitKb: number | null;
}

export interface Account {
  id: string;
  displayName: string;
  email: string;
  /** "imap" for a real mailbox, "local" for imported old mail. */
  kind: "imap" | "local";
  quota?: Quota | null;
  signature?: string | null;
  /** Last error while checking this account, if the last check failed. */
  lastError?: VemError | null;
}

export interface Folder {
  id: string;
  accountId: string;
  /** Name on the server ("INBOX", "[Gmail]/Prullenbak"). */
  name: string;
  role?: FolderRole | null;
  unread: number;
  total: number;
  /** Path parts for nesting ("Werk", "Facturen"). */
  path: string[];
}

export interface MessageSummary {
  id: string;
  accountId: string;
  folderId: string;
  from: Addr;
  to: Addr[];
  subject: string;
  /** ISO 8601. */
  date: string;
  preview: string;
  unread: boolean;
  important: boolean;
  hasAttachments: boolean;
  tagIds: string[];
}

export interface Attachment {
  id: string;
  name: string;
  size: number;
  mime: string;
}

export type WarningKind = "linkMismatch" | "lookalikeContact" | "firstContactMoney";

export interface MailWarning {
  kind: WarningKind;
  /** Plain data for the sentence: the shown and real address of a link, the known contact name. */
  shown?: string | null;
  actual?: string | null;
}

export interface MessageDetail extends MessageSummary {
  cc: Addr[];
  replyTo: Addr[];
  /** Already sanitised by the core. Shown only inside the sandboxed frame. */
  html?: string | null;
  text?: string | null;
  hasRemoteImages: boolean;
  remoteImagesShown: boolean;
  attachments: Attachment[];
  warnings: MailWarning[];
  messageIdHeader?: string | null;
  references: string[];
}

export type SearchScope =
  | { kind: "all" }
  | { kind: "account"; accountId: string }
  | { kind: "folder"; folderId: string };

export interface SearchHit {
  message: MessageSummary;
  accountName: string;
  folderName: string;
}

export interface Tag {
  id: string;
  name: string;
  /** One of the fixed palette keys, see src/ui/tags.ts. */
  color: string;
}

export interface Contact {
  id: string;
  name: string;
  emails: string[];
  phone?: string | null;
  note?: string | null;
}

export interface Draft {
  id?: string | null;
  accountId: string;
  to: Addr[];
  cc: Addr[];
  bcc: Addr[];
  subject: string;
  bodyText: string;
  /** Paths of files on this computer. */
  attachments: string[];
  inReplyTo?: string | null;
  references: string[];
  /** C1: shrink large photos before sending. */
  shrinkPhotos: boolean;
}

export interface OutboxItem {
  id: string;
  subject: string;
  /** ISO time the send really starts (undo send, C3). */
  sendAt: string;
  state: "waiting" | "sending" | "failed";
  error?: VemError | null;
}

export interface VemError {
  /** Fixed code, for example "VEM-IMAP-LOGIN". */
  code: string;
  /** The server's exact reply, if there was one. Never translated. */
  serverReply?: string | null;
  /** Extra plain data for the sentence, for example a host name. */
  detail?: string | null;
}

export interface ServerSettings {
  imapHost: string;
  imapPort: number;
  imapTls: "tls" | "starttls";
  smtpHost: string;
  smtpPort: number;
  smtpTls: "tls" | "starttls";
  username: string;
  /** Where the settings came from: "table", "autoconfig", "ispdb", "guess". */
  source: string;
  /** True when the host is a guess outside the user's domain and the user must confirm it. */
  needsConfirmation: boolean;
  signIn: "password" | "google" | "microsoft";
}

export interface Settings {
  /** Body text size in px, 18 to 32. */
  textSize: number;
  notifications: boolean;
  startWithWindows: boolean;
  /** C3: seconds before a send really starts. 0 turns it off. */
  undoSendSeconds: number;
  /** C1 default for new messages. */
  shrinkPhotos: boolean;
  /** Set once the user has been told about reports to Martijn. */
  reportNoticeShown: boolean;
  /** Set once the user has been told that closing keeps the app next to the clock. */
  trayNoticeShown: boolean;
}

export type ImportKind = "pst" | "wlm" | "dbx" | "eml" | "mbox";

export interface ImportSource {
  kind: ImportKind;
  path: string;
  /** Bytes, when known. */
  size?: number | null;
  /** Shown name, for example the PST file name. */
  label: string;
}

export interface ImportProgress {
  jobId: string;
  done: number;
  total: number | null;
  currentFolder?: string | null;
  finished: boolean;
  error?: VemError | null;
}

export interface OriginalCopy {
  id: string;
  label: string;
  size: number;
  /** ISO date of the import. */
  importedAt: string;
}

export interface UndoToken {
  token: string;
  count: number;
}

export interface AppInfo {
  version: string;
  /** Set for one day after an update. */
  updatedTo?: string | null;
  whatsNew?: string[] | null;
}

/** Events the core sends to the screens. */
export interface CoreEvents {
  "mail:changed": { accountId: string; folderId?: string | null };
  "sync:state": { accountId: string; busy: boolean; error?: VemError | null };
  "import:progress": ImportProgress;
  "outbox:changed": OutboxItem[];
  "open:message": { messageId: string };
  "open:compose": { to: string };
}

/** Where imported mail goes (start_import). Added by the screens. */
export type ImportTarget = { kind: "local" } | { kind: "mailbox"; accountId: string };

/** Result of pair_phone. Added by the screens. */
export interface PairInfo {
  /** SVG markup of the QR code; shown only as an image, never as HTML. */
  qrSvg: string;
}
