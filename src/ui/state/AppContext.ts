import { createContext, useContext } from "react";
import type {
  Account,
  AppInfo,
  Draft,
  Folder,
  OutboxItem,
  Settings,
  Tag,
  VemError,
} from "../../api/types";

export type MailLocation =
  | { kind: "folder"; folderId: string }
  | { kind: "tag"; tagId: string }
  | { kind: "outbox" };

export type View =
  | { kind: "mail" }
  | { kind: "compose"; draft: Draft; key: number }
  | { kind: "contacts" }
  | { kind: "settings" }
  | { kind: "help" }
  | { kind: "import" }
  | { kind: "phone" }
  | { kind: "setup" };

export interface UndoNotice {
  id: number;
  text: string;
  onUndo: () => Promise<void>;
}

export interface AppContextValue {
  ready: boolean;
  appInfo: AppInfo | null;
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
  accounts: Account[];
  folders: Folder[];
  tags: Tag[];
  outbox: OutboxItem[];
  syncErrors: Readonly<Record<string, VemError>>;
  /** Goes up whenever mail changed, so lists know to load again. */
  mailVersion: number;
  reload: () => Promise<void>;

  view: View;
  go: (view: View) => void;
  location: MailLocation;
  setLocation: (location: MailLocation) => void;
  openMessageId: string | null;
  setOpenMessageId: (id: string | null) => void;

  compose: (draft: Draft) => void;
  newDraft: (patch?: Partial<Draft>) => Draft;
  pendingSend: OutboxItem | null;
  startSend: (item: OutboxItem) => void;
  clearSend: () => void;

  error: VemError | null;
  showError: (e: unknown) => void;
  dismissError: () => void;
  undoNotice: UndoNotice | null;
  showUndo: (text: string, onUndo: () => Promise<void>) => void;
  dismissUndo: () => void;
}

export const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const value = useContext(AppContext);
  if (value === null) throw new Error("useApp outside AppProvider");
  return value;
}

export const DEFAULT_SETTINGS: Settings = {
  textSize: 18,
  notifications: true,
  startWithWindows: true,
  undoSendSeconds: 10,
  shrinkPhotos: true,
  reportNoticeShown: false,
  trayNoticeShown: false,
};

export const MIN_TEXT_SIZE = 18;
export const MAX_TEXT_SIZE = 32;
