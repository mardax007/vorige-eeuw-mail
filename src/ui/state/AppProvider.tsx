import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api } from "../../api/client";
import { ALL_INBOXES_ID } from "../../api/commands";
import { toVemError } from "../../api/errors";
import type { Account, AppInfo, Draft, Folder, OutboxItem, Settings, Tag, VemError } from "../../api/types";
import { mailAccounts } from "../folders";
import {
  AppContext,
  DEFAULT_SETTINGS,
  type AppContextValue,
  type MailLocation,
  type UndoNotice,
  type View,
} from "./AppContext";
import { sendShellTexts } from "./shellTexts";
import { useCoreEvents } from "./useCoreEvents";

/** How long the "Ongedaan maken" bar stays (docs/screens.md: about ten seconds). */
export const UNDO_MS = 10_000;

interface AppProviderProps {
  children: ReactNode;
}

function firstInbox(accounts: readonly Account[], folders: readonly Folder[]): MailLocation {
  const mail = mailAccounts(accounts);
  if (mail.length >= 2) return { kind: "folder", folderId: ALL_INBOXES_ID };
  const inbox = folders.find((f) => f.role === "inbox" && f.accountId === mail[0]?.id);
  return { kind: "folder", folderId: inbox?.id ?? ALL_INBOXES_ID };
}

export function AppProvider({ children }: AppProviderProps) {
  const [ready, setReady] = useState(false);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [outbox, setOutbox] = useState<OutboxItem[]>([]);
  const [syncErrors, setSyncErrors] = useState<Record<string, VemError>>({});
  const [syncBusy, setSyncBusy] = useState<Record<string, boolean>>({});
  const [locationPick, setLocationPick] = useState(0);
  const [mailVersion, setMailVersion] = useState(0);
  const [view, setView] = useState<View>({ kind: "mail" });
  const [location, setLocationState] = useState<MailLocation>({ kind: "folder", folderId: ALL_INBOXES_ID });
  const [openMessageId, setOpenMessageId] = useState<string | null>(null);
  const [pendingSend, setPendingSend] = useState<OutboxItem | null>(null);
  const [error, setError] = useState<VemError | null>(null);
  const [undoNotice, setUndoNotice] = useState<UndoNotice | null>(null);
  const undoTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const composeKey = useRef(0);
  const undoId = useRef(0);

  const showError = useCallback((e: unknown) => setError(toVemError(e)), []);

  const reload = useCallback(async () => {
    try {
      const [acc, fol, tg, out] = await Promise.all([
        api.listAccounts(),
        api.listFolders(null),
        api.listTags(),
        api.listOutbox(),
      ]);
      setAccounts(acc);
      setFolders(fol);
      setTags(tg);
      setOutbox(out);
      setMailVersion((v) => v + 1);
    } catch (e: unknown) {
      showError(e);
    }
  }, [showError]);

  useEffect(() => {
    sendShellTexts();
  }, []);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const [info, set, acc, fol, tg, out] = await Promise.all([
          api.appInfo(),
          api.getSettings(),
          api.listAccounts(),
          api.listFolders(null),
          api.listTags(),
          api.listOutbox(),
        ]);
        if (!alive) return;
        setAppInfo(info);
        setSettings(set);
        setAccounts(acc);
        setFolders(fol);
        setTags(tg);
        setOutbox(out);
        setLocationState(firstInbox(acc, fol));
        setView(acc.length === 0 ? { kind: "setup" } : { kind: "mail" });
      } catch (e: unknown) {
        if (alive) showError(e);
      } finally {
        if (alive) setReady(true);
      }
    })();
    return () => {
      alive = false;
    };
  }, [showError]);

  useEffect(() => {
    document.documentElement.style.setProperty("--text-size", `${settings.textSize}px`);
  }, [settings.textSize]);

  const updateSettings = useCallback(
    async (patch: Partial<Settings>) => {
      const next = { ...settings, ...patch };
      setSettings(next);
      try {
        await api.saveSettings(next);
      } catch (e: unknown) {
        showError(e);
      }
    },
    [settings, showError],
  );

  const dismissUndo = useCallback(() => {
    if (undoTimer.current) clearTimeout(undoTimer.current);
    undoTimer.current = null;
    setUndoNotice(null);
  }, []);

  const showUndo = useCallback(
    (text: string, onUndo: () => Promise<void>) => {
      if (undoTimer.current) clearTimeout(undoTimer.current);
      undoId.current += 1;
      setUndoNotice({ id: undoId.current, text, onUndo });
      undoTimer.current = setTimeout(() => setUndoNotice(null), UNDO_MS);
    },
    [],
  );

  useEffect(() => () => {
    if (undoTimer.current) clearTimeout(undoTimer.current);
  }, []);

  const setLocation = useCallback((next: MailLocation) => {
    setLocationState(next);
    setLocationPick((n) => n + 1);
    setOpenMessageId(null);
  }, []);

  const compose = useCallback((draft: Draft) => {
    composeKey.current += 1;
    setView({ kind: "compose", draft, key: composeKey.current });
  }, []);

  const newDraft = useCallback(
    (patch?: Partial<Draft>): Draft => {
      const mail = mailAccounts(accounts);
      const current = location.kind === "folder" ? folders.find((f) => f.id === location.folderId) : undefined;
      const account = mail.find((a) => a.id === current?.accountId) ?? mail[0];
      const signature = account?.signature ? `\n\n${account.signature}` : "";
      return {
        id: null,
        accountId: account?.id ?? "",
        to: [],
        cc: [],
        bcc: [],
        subject: "",
        bodyText: signature,
        attachments: [],
        inReplyTo: null,
        references: [],
        shrinkPhotos: settings.shrinkPhotos,
        ...patch,
      };
    },
    [accounts, folders, location, settings.shrinkPhotos],
  );

  useCoreEvents({
    reload,
    setOutbox,
    setSyncErrors,
    setSyncBusy,
    showMessage: (folderId, messageId) => {
      setLocationState({ kind: "folder", folderId });
      setOpenMessageId(messageId);
      setView({ kind: "mail" });
    },
    composeTo: (to) => compose(newDraft({ to })),
    showError,
  });

  const value = useMemo<AppContextValue>(
    () => ({
      ready,
      appInfo,
      settings,
      updateSettings,
      accounts,
      folders,
      tags,
      outbox,
      syncErrors,
      isSyncing: Object.keys(syncBusy).length > 0,
      mailVersion,
      reload,
      view,
      go: setView,
      location,
      setLocation,
      locationPick,
      openMessageId,
      setOpenMessageId,
      compose,
      newDraft,
      pendingSend,
      startSend: setPendingSend,
      clearSend: () => setPendingSend(null),
      error,
      showError,
      dismissError: () => setError(null),
      undoNotice,
      showUndo,
      dismissUndo,
    }),
    [
      ready, appInfo, settings, updateSettings, accounts, folders, tags, outbox, syncErrors, syncBusy,
      mailVersion, reload, view, location, setLocation, locationPick, openMessageId, compose, newDraft,
      pendingSend, error, showError, undoNotice, showUndo, dismissUndo,
    ],
  );

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}
