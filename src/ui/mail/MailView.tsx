import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { ALL_INBOXES_ID } from "../../api/commands";
import type { Folder, MessageSummary } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { ErrorBox } from "../common/ErrorBox";
import { useBackStep } from "../common/useBackStep";
import { folderLabel } from "../folders";
import { useApp, type MailLocation } from "../state/AppContext";
import { FolderPane } from "./FolderPane";
import { MessageList, type ListItem } from "./MessageList";
import { OutboxView } from "./OutboxView";
import { ReadingPane } from "./ReadingPane";
import { availableChoices, scopeFor, SearchBar, useSearch, type ScopeChoice } from "./SearchBar";
import { useMailKeys } from "./useMailKeys";
import { useMessages } from "./useMessages";
import { useScreenSize } from "./useNarrow";

function locationTitle(location: MailLocation, folder: Folder | undefined, tagName: string | undefined): string {
  if (location.kind === "outbox") return t("folder.outbox");
  if (location.kind === "tag") return tagName ?? t("folder.tags");
  if (location.kind === "domain") return t("folder.domainInbox", { domain: location.domain });
  if (location.folderId === ALL_INBOXES_ID) return t("folder.allInboxes");
  return folder ? folderLabel(folder) : "";
}

/** After "Terug" on a narrow screen: focus the row that was open, so the place in the list stays. */
function focusRow(id: string) {
  requestAnimationFrame(() => {
    const rows = document.querySelectorAll<HTMLButtonElement>("button.message-row[data-id]");
    for (const row of Array.from(rows)) {
      if (row.dataset.id === id) {
        row.focus();
        return;
      }
    }
  });
}

function defaultChoice(choices: readonly ScopeChoice[]): ScopeChoice {
  return choices.includes("folder") ? "folder" : "all";
}

/** The main screen: folders, the list, the open message. */
export function MailView() {
  const app = useApp();
  const { location, locationPick, folders, accounts, tags, mailVersion, openMessageId, setOpenMessageId, showError } = app;
  const { isNarrow, isPhone } = useScreenSize(app.settings.textSize);
  const folder = location.kind === "folder" ? folders.find((f) => f.id === location.folderId) : undefined;
  const choices = availableChoices(location, folder);
  const locationKey = JSON.stringify(location);
  const [query, setQuery] = useState("");
  const [choice, setChoice] = useState<ScopeChoice>(defaultChoice(choices));
  const [isConfirmingEmpty, setIsConfirmingEmpty] = useState(false);
  const [exportedTo, setExportedTo] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  // On a phone the folders are a screen of their own, opened with "Mappen".
  const [isPickingFolder, setIsPickingFolder] = useState(false);

  useEffect(() => {
    // Picking a folder, also the same one again, ends a search.
    setQuery("");
    setChoice(defaultChoice(availableChoices(location, folder)));
    setExportedTo(null);
    setIsPickingFolder(false);
    // Reset only when the place changes or is picked again, not when its counts change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [locationKey, locationPick]);

  const list = useMessages(location, mailVersion, showError);
  const search = useSearch(query, scopeFor(choice, location, folder), showError);
  useMailKeys(openMessageId);

  const accountName = (id: string) => accounts.find((a) => a.id === id)?.displayName ?? "";
  const folderName = (id: string, fallback: string) => {
    const f = folders.find((x) => x.id === id);
    return f ? folderLabel(f) : fallback;
  };
  const isSearching = query.trim() !== "";
  const showWhere = location.kind === "tag" || (location.kind === "folder" && location.folderId === ALL_INBOXES_ID);
  const items: ListItem[] = isSearching
    ? search.hits.map((h) => ({
        message: h.message,
        where: t("search.where.hit", { account: h.accountName, folder: folderName(h.message.folderId, h.folderName) }),
      }))
    : list.messages.map((m) => ({ message: m, where: showWhere ? t("list.inAccount", { account: accountName(m.accountId) }) : undefined }));

  const isLocalFolder = folder !== undefined && accounts.find((a) => a.id === folder.accountId)?.kind === "local";
  const handleExport = async () => {
    if (!folder || isBusy) return;
    setIsBusy(true);
    try {
      setExportedTo(await api.exportFolder(folder.id));
    } catch (e: unknown) {
      showError(e);
    } finally {
      setIsBusy(false);
    }
  };
  const handleEmptyTrash = async () => {
    setIsConfirmingEmpty(false);
    if (!folder || isBusy) return;
    setIsBusy(true);
    try {
      await api.emptyTrash(folder.accountId);
      setOpenMessageId(null);
      await app.reload();
    } catch (e: unknown) {
      showError(e);
    } finally {
      setIsBusy(false);
    }
  };

  const actions = (
    <>
      {!isSearching && folder?.role === "trash" && folder.total > 0 && (
        <button type="button" className="danger-outline" disabled={isBusy} onClick={() => setIsConfirmingEmpty(true)}>
          {t("folder.emptyTrash")}
        </button>
      )}
      {!isSearching && isLocalFolder && (
        <button type="button" disabled={isBusy} onClick={handleExport}>
          {isBusy ? t("general.saving") : t("folder.export")}
        </button>
      )}
    </>
  );

  // One mailbox shows its own error; "Alle postvakken", a label list or Postvak UIT show every mailbox's.
  const syncErrors = Object.entries(app.syncErrors)
    .filter(([accountId]) => folder === undefined || accountId === folder.accountId)
    .map(([accountId, error]) => ({ accountId, error }));
  const isAccountNamed = folder === undefined;
  const open = (m: MessageSummary) => setOpenMessageId(m.id);
  const isOverList = isNarrow && openMessageId !== null;
  const isFolderScreen = isPhone && isPickingFolder && !isOverList;
  const showFolders = !isOverList && (!isPhone || isFolderScreen);
  const closeMessage = () => {
    const id = openMessageId;
    setOpenMessageId(null);
    if (id) focusRow(id);
  };
  // Android's back key: the open message first, then the folder screen.
  useBackStep(isOverList, closeMessage, 1);
  useBackStep(isFolderScreen, () => setIsPickingFolder(false), 1);

  let emptyText = t("list.empty");
  if (isSearching) emptyText = search.isBusy ? t("search.busy") : t("search.none", { query: query.trim() });
  else if (location.kind === "tag") emptyText = t("list.emptyTag");
  else if (app.isSyncing) emptyText = t("list.emptySyncing");

  const middle =
    location.kind === "outbox" && !isSearching ? (
      <OutboxView />
    ) : (
      <MessageList
        title={isSearching ? t("search.results") : locationTitle(location, folder, tags.find((x) => location.kind === "tag" && x.id === location.tagId)?.name)}
        items={items}
        tags={tags}
        selectedId={openMessageId}
        isLoading={isSearching ? search.isBusy : list.isLoading}
        loadError={isSearching ? search.loadError : list.loadError}
        onRetry={isSearching ? search.retry : list.retry}
        emptyText={emptyText}
        loadingText={isSearching ? t("search.busy") : undefined}
        failedText={isSearching ? t("search.failed") : undefined}
        hasMore={!isSearching && list.hasMore}
        onLoadMore={list.loadMore}
        onOpen={open}
        actions={actions}
      />
    );

  return (
    <div className={`mail-view${isNarrow ? " narrow" : ""}${isPhone ? " phone" : ""}`}>
      {isFolderScreen && (
        <button type="button" className="back-button phone-back" onClick={() => setIsPickingFolder(false)}>
          {t("small.backToList")}
        </button>
      )}
      {showFolders && <FolderPane />}
      {!isFolderScreen && (
      <div className="mail-main">
        {isPhone && !isOverList && (
          <button type="button" className="folders-button" onClick={() => setIsPickingFolder(true)}>
            {t("small.folders")}
          </button>
        )}
        {!isOverList && (
          <SearchBar query={query} choice={choice} choices={choices} onQuery={setQuery} onChoice={setChoice} />
        )}
        {!isOverList &&
          syncErrors.map(({ accountId, error }) => (
            <div key={accountId} className="sync-error">
              {isAccountNamed && <h3 className="sync-error-account">{accountName(accountId)}</h3>}
              <ErrorBox error={error} />
            </div>
          ))}
        {exportedTo && !isOverList && <p className="calm-line" role="status">{t("folder.exported", { path: exportedTo })}</p>}
        <div className="mail-columns">
          {/* Kept in the page while a message lies over it, so "Terug" returns to the same place. */}
          <div className="list-holder" hidden={isOverList}>
            {middle}
          </div>
          {openMessageId && (
            <ReadingPane
              messageId={openMessageId}
              onBack={isNarrow ? closeMessage : undefined}
              onRemoved={() => setOpenMessageId(null)}
            />
          )}
          {!openMessageId && !isNarrow && (
            <section className="reading-pane empty">
              <p className="list-note">{t("read.nothingOpen")}</p>
            </section>
          )}
        </div>
      </div>
      )}
      {isConfirmingEmpty && (
        <ConfirmDialog
          text={t("folder.emptyTrashConfirm")}
          confirmLabel={t("folder.emptyTrashYes")}
          onConfirm={handleEmptyTrash}
          onCancel={() => setIsConfirmingEmpty(false)}
        />
      )}
    </div>
  );
}
