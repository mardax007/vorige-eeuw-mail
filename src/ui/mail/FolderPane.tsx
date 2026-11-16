import { useState } from "react";
import { ALL_INBOXES_ID } from "../../api/commands";
import type { Account, Folder } from "../../api/types";
import { t } from "../../i18n";
import { TagChip } from "../common/TagChip";
import { folderLabel, localAccounts, mailAccounts, mainFolders, otherFolders } from "../folders";
import { useApp, type MailLocation } from "../state/AppContext";

interface FolderButtonProps {
  label: string;
  unread: number;
  isCurrent: boolean;
  onSelect: () => void;
}

function FolderButton({ label, unread, isCurrent, onSelect }: FolderButtonProps) {
  return (
    <li>
      <button
        type="button"
        className={`folder-button${isCurrent ? " current" : ""}`}
        aria-current={isCurrent ? "true" : undefined}
        onClick={onSelect}
      >
        <span className="folder-name">{label}</span>
        {unread > 0 && (
          <span className="folder-count" aria-label={t("folder.unreadCount", { count: unread })}>
            {unread}
          </span>
        )}
      </button>
    </li>
  );
}

function isAt(location: MailLocation, folderId: string): boolean {
  return location.kind === "folder" && location.folderId === folderId;
}

interface FolderGroupProps {
  title: string;
  folders: Folder[];
}

function FolderGroup({ title, folders }: FolderGroupProps) {
  const { location, setLocation, go } = useApp();
  if (folders.length === 0) return null;
  return (
    <div className="folder-group">
      <h3>{title}</h3>
      <ul>
        {folders.map((f) => (
          <FolderButton
            key={f.id}
            label={folderLabel(f)}
            unread={f.role === "trash" || f.role === "junk" ? 0 : f.unread}
            isCurrent={isAt(location, f.id)}
            onSelect={() => {
              setLocation({ kind: "folder", folderId: f.id });
              go({ kind: "mail" });
            }}
          />
        ))}
      </ul>
    </div>
  );
}

function allInboxesUnread(accounts: readonly Account[], folders: readonly Folder[]): number {
  const ids = new Set(mailAccounts(accounts).map((a) => a.id));
  return folders.filter((f) => f.role === "inbox" && ids.has(f.accountId)).reduce((n, f) => n + f.unread, 0);
}

/** Left pane: the few folders people use, and the rest behind "Meer mappen". */
export function FolderPane() {
  const { accounts, folders, tags, outbox, location, setLocation, go } = useApp();
  const [isMoreOpen, setIsMoreOpen] = useState(false);
  const mail = mailAccounts(accounts);
  const local = localAccounts(accounts);
  const select = (next: MailLocation) => {
    setLocation(next);
    go({ kind: "mail" });
  };

  return (
    <nav className="folder-pane" aria-label={t("folder.pane")}>
      {mail.length >= 2 && (
        <ul>
          <FolderButton
            label={t("folder.allInboxes")}
            unread={allInboxesUnread(accounts, folders)}
            isCurrent={isAt(location, ALL_INBOXES_ID)}
            onSelect={() => select({ kind: "folder", folderId: ALL_INBOXES_ID })}
          />
        </ul>
      )}
      {mail.map((a) => (
        <FolderGroup key={a.id} title={a.displayName} folders={mainFolders(folders, a.id)} />
      ))}
      {outbox.length > 0 && (
        <ul>
          <FolderButton
            label={t("folder.outbox")}
            unread={outbox.length}
            isCurrent={location.kind === "outbox"}
            onSelect={() => select({ kind: "outbox" })}
          />
        </ul>
      )}
      <button
        type="button"
        className="more-button"
        aria-expanded={isMoreOpen}
        onClick={() => setIsMoreOpen(!isMoreOpen)}
      >
        {isMoreOpen ? t("folder.less") : t("folder.more")}
      </button>
      {isMoreOpen && (
        <div className="more-folders">
          {mail.map((a) => (
            <FolderGroup key={a.id} title={a.displayName} folders={otherFolders(folders, a.id)} />
          ))}
          {tags.length > 0 && (
            <div className="folder-group">
              <h3>{t("folder.tags")}</h3>
              <ul>
                {tags.map((tag) => (
                  <li key={tag.id}>
                    <button
                      type="button"
                      className={`folder-button${location.kind === "tag" && location.tagId === tag.id ? " current" : ""}`}
                      onClick={() => select({ kind: "tag", tagId: tag.id })}
                    >
                      <TagChip tag={tag} />
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {local.map((a) => (
            <FolderGroup key={a.id} title={a.displayName} folders={folders.filter((f) => f.accountId === a.id)} />
          ))}
          <ul className="more-links">
            <li>
              <button type="button" className="folder-button" onClick={() => go({ kind: "contacts" })}>
                {t("nav.contacts")}
              </button>
            </li>
            <li>
              <button type="button" className="folder-button" onClick={() => go({ kind: "import" })}>
                {t("nav.import")}
              </button>
            </li>
            <li>
              <button type="button" className="folder-button" onClick={() => go({ kind: "phone" })}>
                {t("nav.phone")}
              </button>
            </li>
          </ul>
        </div>
      )}
    </nav>
  );
}
