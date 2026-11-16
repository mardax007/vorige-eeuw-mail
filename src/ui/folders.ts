import type { Account, Folder, FolderRole } from "../api/types";
import { t, type MessageKey } from "../i18n";

const ROLE_KEYS: Record<FolderRole, MessageKey> = {
  inbox: "folder.inbox",
  sent: "folder.sent",
  trash: "folder.trash",
  drafts: "folder.drafts",
  junk: "folder.junk",
  archive: "folder.archive",
  outbox: "folder.outbox",
};

/** Outlook's Dutch word for a recognised folder; other folders keep their own name. */
export function folderLabel(folder: Pick<Folder, "role" | "name" | "path">): string {
  if (folder.role) return t(ROLE_KEYS[folder.role]);
  return folder.path.length > 0 ? folder.path.join(" / ") : folder.name;
}

/** Shown on the main screen for each mailbox; everything else is behind "Meer mappen". */
export const MAIN_ROLES: readonly FolderRole[] = ["inbox", "sent", "trash"];

export function mainFolders(folders: readonly Folder[], accountId: string): Folder[] {
  return MAIN_ROLES.flatMap((role) =>
    folders.filter((f) => f.accountId === accountId && f.role === role),
  );
}

export function otherFolders(folders: readonly Folder[], accountId: string): Folder[] {
  return folders
    .filter((f) => f.accountId === accountId && !(f.role && MAIN_ROLES.includes(f.role)))
    .sort((a, b) => folderLabel(a).localeCompare(folderLabel(b), "nl"));
}

export function mailAccounts(accounts: readonly Account[]): Account[] {
  return accounts.filter((a) => a.kind === "imap");
}

export function localAccounts(accounts: readonly Account[]): Account[] {
  return accounts.filter((a) => a.kind === "local");
}
