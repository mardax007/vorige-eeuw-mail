import type {
  Account,
  Contact,
  Draft,
  Folder,
  MessageDetail,
  OriginalCopy,
  OutboxItem,
  Settings,
  Tag,
} from "../types";
import type { MockEvents } from "./events";

/** Folder as the mock keeps it; unread and total are counted when listed. */
export type MockFolder = Omit<Folder, "unread" | "total">;

export interface UndoEntry {
  /** Messages as they were before the change. */
  before: MessageDetail[];
}

export interface MockOutbox {
  item: OutboxItem;
  draft: Draft;
}

export interface MockState {
  accounts: Account[];
  folders: MockFolder[];
  messages: MessageDetail[];
  tags: Tag[];
  contacts: Contact[];
  drafts: Record<string, Draft>;
  outbox: MockOutbox[];
  undo: Record<string, UndoEntry>;
  settings: Settings;
  originals: OriginalCopy[];
}

export interface MockStore {
  get(): MockState;
  set(update: (state: MockState) => MockState): void;
  nextId(prefix: string): string;
  events: MockEvents;
  later(ms: number, run: () => void): ReturnType<typeof setTimeout>;
  cancel(timer: ReturnType<typeof setTimeout>): void;
}

export function mockError(code: string, serverReply?: string, detail?: string) {
  return { code, serverReply: serverReply ?? null, detail: detail ?? null };
}

export function folderOf(state: MockState, folderId: string): MockFolder | undefined {
  return state.folders.find((f) => f.id === folderId);
}

export function roleFolder(
  state: MockState,
  accountId: string,
  role: string,
): MockFolder | undefined {
  return state.folders.find((f) => f.accountId === accountId && f.role === role);
}

export function newestFirst<T extends { date: string }>(list: readonly T[]): T[] {
  return [...list].sort((a, b) => b.date.localeCompare(a.date));
}
