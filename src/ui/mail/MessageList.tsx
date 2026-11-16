import type { ReactNode } from "react";
import type { MessageSummary, Tag } from "../../api/types";
import { t } from "../../i18n";
import { TagChip } from "../common/TagChip";
import { displayName, shortDate } from "../format";

export interface ListItem {
  message: MessageSummary;
  /** Mailbox and folder, shown for search results and "Alle postvakken". */
  where?: string;
}

interface MessageRowProps {
  item: ListItem;
  tags: readonly Tag[];
  isSelected: boolean;
  onOpen: (message: MessageSummary) => void;
}

function MessageRow({ item, tags, isSelected, onOpen }: MessageRowProps) {
  const m = item.message;
  const rowTags = tags.filter((tag) => m.tagIds.includes(tag.id));
  const classes = ["message-row", m.unread ? "unread" : "", isSelected ? "selected" : ""].join(" ");
  return (
    <li>
      <button
        type="button"
        className={classes}
        aria-current={isSelected ? "true" : undefined}
        onClick={() => onOpen(m)}
      >
        <span className="row-top">
          <span className="row-sender">
            {m.unread && <span className="sr-only">{t("list.unread")}: </span>}
            {displayName(m.from)}
          </span>
          <span className="row-date">{shortDate(m.date)}</span>
        </span>
        <span className="row-address"> &lt;{m.from.email}&gt;</span>
        <span className="row-subject">{m.subject || t("list.noSubject")}</span>
        <span className="row-extras">
          {m.important && <span className="row-important">★ {t("list.important")}</span>}
          {m.hasAttachments && <span className="row-attachment">{t("list.attachment")}</span>}
          {rowTags.map((tag) => (
            <TagChip key={tag.id} tag={tag} />
          ))}
          {item.where && <span className="row-where">{item.where}</span>}
        </span>
      </button>
    </li>
  );
}

interface MessageListProps {
  title: string;
  items: readonly ListItem[];
  tags: readonly Tag[];
  selectedId: string | null;
  isLoading: boolean;
  emptyText: string;
  hasMore?: boolean;
  onLoadMore?: () => void;
  onOpen: (message: MessageSummary) => void;
  actions?: ReactNode;
}

/** The middle column: one row per message, unread in bold. */
export function MessageList({
  title,
  items,
  tags,
  selectedId,
  isLoading,
  emptyText,
  hasMore = false,
  onLoadMore,
  onOpen,
  actions,
}: MessageListProps) {
  return (
    <section className="message-list" aria-label={t("list.messages")}>
      <header className="list-header">
        <h2>{title}</h2>
        {actions && <div className="button-row">{actions}</div>}
      </header>
      {isLoading && items.length === 0 && <p className="list-note">{t("list.loading")}</p>}
      {!isLoading && items.length === 0 && <p className="list-note">{emptyText}</p>}
      <ul className="rows">
        {items.map((item) => (
          <MessageRow
            key={item.message.id}
            item={item}
            tags={tags}
            isSelected={item.message.id === selectedId}
            onOpen={onOpen}
          />
        ))}
      </ul>
      {hasMore && onLoadMore && (
        <button type="button" className="load-more" onClick={onLoadMore}>
          {t("list.more")}
        </button>
      )}
    </section>
  );
}
