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
  /** How this mailbox signs in; null for local mailboxes (and older indexes). */
  signIn?: SignIn | null;
  /**
   * Other addresses that arrive in this mailbox, such as custom-domain addresses on iCloud
   * (decision 32); lower case, without the main address.
   */
  addresses: string[];
}

/** Postvak IN of one mailbox, only the mail addressed to one domain of its other addresses. */
export interface DomainInbox {
  accountId: string;
  domain: string;
  unread: number;
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

/** A moment as the invitation wrote it; the screen turns it into the device's own time. */
export interface InviteTime {
  /** "2026-11-20T14:00:00", or "2026-11-20" for a whole day. */
  wall: string;
  /** "UTC", an IANA zone, the invitation's own zone name, or null for a floating time. */
  zone?: string | null;
  allDay: boolean;
}

/** An invitation in a message (C11): shown readably, never answered from the app. */
export interface Invite {
  summary?: string | null;
  start?: InviteTime | null;
  end?: InviteTime | null;
  location?: string | null;
  organizer?: Addr | null;
  cancelled: boolean;
  repeats: boolean;
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
  invite?: Invite | null;
  /** Where the server delivered the message (Delivered-To), lower case (decision 32). */
  deliveredTo?: string[];
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
  /** The text with its formatting and photos (decision 33); null when it has none. */
  bodyHtml?: string | null;
  /** Paths of files on this computer. */
  attachments: string[];
  inReplyTo?: string | null;
  references: string[];
  /** C1: shrink large photos before sending. */
  shrinkPhotos: boolean;
  /** The address to send from when the mailbox has more than one (decision 32). */
  fromAddress?: string | null;
}

/** A photo picked for the text: a data: URL, or null when the file is no photo the app can read. */
export interface TextPhoto {
  dataUrl: string | null;
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
  /** A report about this error went to Martijn's queue. */
  reported?: boolean;
}

export interface ServerSettings {
  imapHost: string;
  imapPort: number;
  imapTls: "tls" | "starttls";
  smtpHost: string;
  smtpPort: number;
  smtpTls: "tls" | "starttls";
  username: string;
  /** Where the settings came from: "table", "autoconfig", "ispdb", "mx", "srv", "guess", "manual". */
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
  /** Android: keep watching for new mail with the app closed (decision 24). Off by default. */
  backgroundMail: boolean;
}

export type ImportKind = "pst" | "wlm" | "dbx" | "eml" | "mbox";

export interface ImportSource {
  kind: ImportKind;
  path: string;
  /** Bytes, when known. */
  size?: number | null;
  /** Shown name, for example the PST file name. */
  label: string;
  /** An Outlook file with a password: ask for it before the import starts. */
  protected?: boolean;
}

/** A folder an import left out, with the core's short technical reason (English; shown only as details). */
export interface ImportSkip {
  folder: string;
  reason: string;
}

export interface ImportProgress {
  jobId: string;
  done: number;
  total: number | null;
  currentFolder?: string | null;
  finished: boolean;
  error?: VemError | null;
  /** Messages that were already in the target and were not copied again. */
  alreadyThere: number;
  /** Contacts from an Outlook file that went into the address book (final event only). */
  contactsAdded?: number;
  /** Windows Live Mail: its read state is not known, so every message came in as read. */
  allMarkedRead?: boolean;
  /** Read with the rescue tool for damaged Outlook files: read state may be lost. */
  rescued?: boolean;
  /** Filled only in the final event (finished = true). */
  skipped: ImportSkip[];
  /** The file or mailbox this import reads, as the screen named it. */
  source?: string | null;
  /** Final event only: the folder the first new message went into. */
  folderId?: string | null;
}

/** Contacts read from vCard files (Windows Live Mail's export). */
export interface ContactsImported {
  /** Cards with an e-mail address in the chosen files. */
  found: number;
  /** Of those, the ones that were not in the address book yet. */
  added: number;
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
  /** The Android app: no "Starten met Windows". */
  isPhone?: boolean;
}

/** Events the core sends to the screens. */
/** One mailbox named in a backup's description. */
export interface BackupAccount {
  name: string;
  email: string;
}

/** A checked backup, shown before "Terugzetten?" (C14). */
export interface BackupInfo {
  path: string;
  createdAt: string;
  accounts: BackupAccount[];
  messages: number;
  bytes: number;
}

/** A backup being made or put back (C14). */
export interface BackupProgress {
  restoring: boolean;
  doneBytes: number;
  totalBytes: number;
  finished: boolean;
  /** "Stoppen" was pressed; nothing half-done is left. */
  stopped: boolean;
  error?: VemError | null;
  /** The last event of a backup: the folder it wrote. */
  folder?: string | null;
}

export interface CoreEvents {
  "mail:changed": { accountId: string; folderId?: string | null };
  "sync:state": { accountId: string; busy: boolean; error?: VemError | null };
  "import:progress": ImportProgress;
  "outbox:changed": OutboxItem[];
  "open:message": { messageId: string };
  /** A `mailto:` link in a mail: its addresses and subject, decoded (empty when it has none). */
  "open:compose": { to: string; subject?: string };
  /** Android: a newer version is on GitHub; the screens offer "Bijwerken". */
  "update:available": UpdateOffer;
  /** Phone: the mail kept on the paired PC coming in (docs/pc-to-phone.md). */
  "lan:phone": PcCopyEvent;
  /** PC: the phone got (or already had) the mail kept on this PC. */
  "lan:sent": { received: number; removed: number; skipped: number };
  /** PC: a backup being made or put back (C14). */
  "backup:progress": BackupProgress;
}

/** What the phone says while the PC's local mail comes in. */
export type PcCopyEvent =
  | { kind: "progress"; done: number; total: number }
  | { kind: "done"; received: number; removed: number }
  | { kind: "stopped" }
  | { kind: "failed"; error: VemError };

/** When the phone last had all mail kept on this PC; `at` in seconds since 1970. */
export interface LastCopy {
  at: number;
  sent: number;
}

/** A newer version waiting for "Bijwerken" (Android only; Windows updates itself). */
export interface UpdateOffer {
  version: string;
  notes: string[];
}

/** Where imported mail goes (start_import). Added by the screens. */
export type ImportTarget = { kind: "local" } | { kind: "mailbox"; accountId: string };

/** Result of pair_phone. Added by the screens. */
export interface PairInfo {
  /** SVG markup of the QR code; shown only as an image, never as HTML. */
  qrSvg: string;
}

/** Screen texts the Rust shell shows itself (tray menu, notifications). Added by the screens. */
export interface ShellTexts {
  /** Tray menu item that opens the window. */
  open: string;
  /** Tray menu item that stops the app. */
  quit: string;
  /** Notification title for one new message. */
  newMail: string;
  /** Notification title for several; the shell replaces {count}. */
  newMailMany: string;
  /** Said once, the first time the window is closed. */
  stillRunning: string;
  /** Android: the permanent notification while the phone watches for new mail. */
  watching: string;
  /** Android: the names of the two kinds of notification, shown in the phone's settings. */
  watchingChannel: string;
  newMailChannel: string;
}

/** How an account signs in (ServerSettings.signIn). Added by the screens. */
export type SignIn = ServerSettings["signIn"];
