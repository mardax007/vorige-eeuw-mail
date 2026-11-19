import { Fragment, useEffect, useState } from "react";
import { ALL_INBOXES_ID } from "../../api/commands";
import { api } from "../../api/client";
import type { Account, DomainInbox, Folder } from "../../api/types";
import { t } from "../../i18n";
import { TagChip } from "../common/TagChip";
import { folderLabel, localAccounts, mailAccounts, mainFolders, otherFolders } from "../folders";
import { useApp, type MailLocation } from "../state/AppContext";

interface FolderButtonProps {
  label: string;
  unread: number;
  isCurrent: boolean;
  onSelect: () => void;
  /** What the number means, read aloud; "{count} ongelezen" unless said otherwise. */
  countLabel?: (count: number) => string;
  className?: string;
}

const unreadLabel = (count: number) => t("folder.unreadCount", { count });

function FolderButton({ label, unread, isCurrent, onSelect, countLabel = unreadLabel, className }: FolderButtonProps) {
  return (
    <li className={className}>
      <button
        type="button"
        className={`folder-button${isCurrent ? " current" : ""}`}
        aria-current={isCurrent ? "true" : undefined}
        onClick={onSelect}
      >
        <span className="folder-name">{label}</span>
        {unread > 0 && (
          <>
            <span className="folder-count" aria-hidden="true">
              {unread}
            </span>
            <span className="sr-only">{countLabel(unread)}</span>
          </>
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
  /** The mailbox's other folders, behind its own "Meer mappen". */
  more?: Folder[];
  /** Postvak IN per domain of the mailbox's other addresses, right under its Postvak IN. */
  domains?: DomainInbox[];
}

/**
 * One mailbox: its main folders, then "Meer mappen (n)", which opens the rest right below, in the
 * same group. The button keeps its words; the arrow and aria-expanded say whether it is open.
 */
function FolderGroup({ title, folders, more = [], domains = [] }: FolderGroupProps) {
  const { location, setLocation, go } = useApp();
  const isInMore = more.some((f) => isAt(location, f.id));
  const [isOpen, setIsOpen] = useState(isInMore);
  useEffect(() => {
    // The open folder is always visible in the pane.
    if (isInMore) setIsOpen(true);
  }, [isInMore]);
  if (folders.length === 0 && more.length === 0) return null;
  const button = (f: Folder) => (
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
  );
  return (
    <div className="folder-group">
      <h3>{title}</h3>
      <ul>
        {folders.map((f) => (
          <Fragment key={f.id}>
            {button(f)}
            {f.role === "inbox" &&
              domains.map((d) => (
                <FolderButton
                  key={d.domain}
                  label={t("folder.domainInbox", { domain: d.domain })}
                  unread={d.unread}
                  isCurrent={location.kind === "domain" && location.accountId === d.accountId && location.domain === d.domain}
                  onSelect={() => {
                    setLocation({ kind: "domain", accountId: d.accountId, domain: d.domain });
                    go({ kind: "mail" });
                  }}
                  className="domain-inbox"
                />
              ))}
          </Fragment>
        ))}
      </ul>
      {more.length > 0 && (
        <>
          <button
            type="button"
            className="more-button"
            aria-expanded={isOpen}
            onClick={() => setIsOpen(!isOpen)}
          >
            <span className="more-arrow" aria-hidden="true">
              {isOpen ? "▼" : "▶"}
            </span>
            <span>{t("folder.moreCount", { count: more.length })}</span>
          </button>
          {isOpen && <ul className="more-folders">{more.map(button)}</ul>}
        </>
      )}
    </div>
  );
}

/**
 * The Postvak IN per domain of the mailboxes' other addresses (decision 32), loaded again when
 * the folders change (new mail, read, moved). Nothing is asked while no mailbox has any.
 */
function useDomainInboxes(accounts: readonly Account[], folders: readonly Folder[]): DomainInbox[] {
  const [list, setList] = useState<DomainInbox[]>([]);
  const hasAddresses = accounts.some((a) => a.addresses.length > 0);
  useEffect(() => {
    let alive = true;
    if (!hasAddresses) {
      setList([]);
      return;
    }
    // A failed count leaves the pane as it was; the mailbox's own Postvak IN still has all mail.
    api.listDomainInboxes().then((l) => alive && setList(l), () => undefined);
    return () => {
      alive = false;
    };
  }, [accounts, folders, hasAddresses]);
  return list;
}

function allInboxesUnread(accounts: readonly Account[], folders: readonly Folder[]): number {
  const ids = new Set(mailAccounts(accounts).map((a) => a.id));
  return folders.filter((f) => f.role === "inbox" && ids.has(f.accountId)).reduce((n, f) => n + f.unread, 0);
}

/**
 * Left pane: per mailbox the folders people use, with that mailbox's other folders behind its own
 * "Meer mappen"; then labels, old mail and the two links, always visible.
 */
export function FolderPane() {
  const { accounts, folders, tags, outbox, location, setLocation, go } = useApp();
  const mail = mailAccounts(accounts);
  const local = localAccounts(accounts);
  const domains = useDomainInboxes(accounts, folders);
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
        <FolderGroup
          key={a.id}
          title={a.displayName}
          folders={mainFolders(folders, a.id)}
          more={otherFolders(folders, a.id)}
          domains={domains.filter((d) => d.accountId === a.id)}
        />
      ))}
      {outbox.length > 0 && (
        <ul>
          <FolderButton
            label={t("folder.outbox")}
            unread={outbox.length}
            countLabel={(count) => t("folder.waitingCount", { count })}
            isCurrent={location.kind === "outbox"}
            onSelect={() => select({ kind: "outbox" })}
          />
        </ul>
      )}
      {tags.length > 0 && (
        <div className="folder-group">
          <h3>{t("folder.tags")}</h3>
          <ul>
            {tags.map((tag) => {
              const isCurrent = location.kind === "tag" && location.tagId === tag.id;
              return (
                <li key={tag.id}>
                  <button
                    type="button"
                    className={`folder-button${isCurrent ? " current" : ""}`}
                    aria-current={isCurrent ? "true" : undefined}
                    onClick={() => select({ kind: "tag", tagId: tag.id })}
                  >
                    <TagChip tag={tag} />
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {local.map((a) => (
        <FolderGroup key={a.id} title={a.displayName} folders={folders.filter((f) => f.accountId === a.id)} />
      ))}
      <ul className="more-links">
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
    </nav>
  );
}
