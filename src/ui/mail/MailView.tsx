import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { ALL_INBOXES_ID } from "../../api/commands";
import type { Folder, MessageSummary } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { ErrorBox } from "../common/ErrorBox";
import { folderLabel } from "../folders";
import { useApp, type MailLocation } from "../state/AppContext";
import { FolderPane } from "./FolderPane";
import { MessageList, type ListItem } from "./MessageList";
import { OutboxView } from "./OutboxView";
import { ReadingPane } from "./ReadingPane";
import { availableChoices, scopeFor, SearchBar, useSearch, type ScopeChoice } from "./SearchBar";
import { useMailKeys } from "./useMailKeys";
import { useMessages } from "./useMessages";
import { useNarrow } from "./useNarrow";

function locationTitle(location: MailLocation, folder: Folder | undefined, tagName: string | undefined): string {
  if (location.kind === "outbox") return t("folder.outbox");
  if (location.kind === "tag") return tagName ?? t("folder.tags");
  if (location.folderId === ALL_INBOXES_ID) return t("folder.allInboxes");
  return folder ? folderLabel(folder) : "";
}

function defaultChoice(choices: readonly ScopeChoice[]): ScopeChoice {
  return choices.includes("folder") ? "folder" : "all";
}

/** The main screen: folders, the list, the open message. */
export function MailView() {
  const app = useApp();
  const { location, locationPick, folders, accounts, tags, mailVersion, openMessageId, setOpenMessageId, showError } = app;
  const isNarrow = useNarrow(app.settings.textSize);
  const folder = location.kind === "folder" ? folders.find((f) => f.id === location.folderId) : undefined;
  const choices = availableChoices(location, folder);
  const locationKey = JSON.stringify(location);
  const [query, setQuery] = useState("");
  const [choice, setChoice] = useState<ScopeChoice>(defaultChoice(choices));
  const [isConfirmingEmpty, setIsConfirmingEmpty] = useState(false);
  const [exportedTo, setExportedTo] = useState<string | null>(null);

  useEffect(() => {
    // Picking a folder, also the same one again, ends a search.
    setQuery("");
    setChoice(defaultChoice(availableChoices(location, folder)));
    setExportedTo(null);
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
    if (!folder) return;
    try {
      setExportedTo(await api.exportFolder(folder.id));
    } catch (e: unknown) {
      showError(e);
    }
  };
  const handleEmptyTrash = async () => {
    setIsConfirmingEmpty(false);
    if (!folder) return;
    try {
      await api.emptyTrash(folder.accountId);
      setOpenMessageId(null);
      await app.reload();
    } catch (e: unknown) {
      showError(e);
    }
  };

  const actions = (
    <>
      {!isSearching && folder?.role === "trash" && folder.total > 0 && (
        <button type="button" onClick={() => setIsConfirmingEmpty(true)}>{t("folder.emptyTrash")}</button>
      )}
      {!isSearching && isLocalFolder && (
        <button type="button" onClick={handleExport}>{t("folder.export")}</button>
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
        emptyText={isSearching ? (search.isBusy ? t("search.busy") : t("search.none", { query: query.trim() })) : t("list.empty")}
        hasMore={!isSearching && list.hasMore}
        onLoadMore={list.loadMore}
        onOpen={open}
        actions={actions}
      />
    );

  return (
    <div className={`mail-view${isNarrow ? " narrow" : ""}`}>
      {!isOverList && <FolderPane />}
      <div className="mail-main">
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
          {!isOverList && middle}
          {openMessageId && (
            <ReadingPane
              messageId={openMessageId}
              onBack={isNarrow ? () => setOpenMessageId(null) : undefined}
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
