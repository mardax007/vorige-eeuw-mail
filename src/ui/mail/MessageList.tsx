import { useEffect, useRef, type ReactNode } from "react";
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
        data-id={m.id}
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
  /** Shown while the first rows load; "Berichten worden geladen…" by default. */
  loadingText?: string;
  /** Shown when nothing could be loaded; the folder's own text by default. */
  failedText?: string;
  /** The list could not be loaded; shows that, with "Opnieuw proberen". */
  loadError?: boolean;
  onRetry?: () => void;
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
  loadingText,
  failedText,
  loadError = false,
  onRetry,
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
      {isLoading && items.length === 0 && (
        <p className="list-note" role="status">{loadingText ?? t("list.loading")}</p>
      )}
      {!isLoading && loadError && (
        <div className="list-note" role="status">
          <p>{items.length > 0 ? t("list.reloadFailed") : (failedText ?? t("list.loadFailed"))}</p>
          {onRetry && (
            <button type="button" onClick={onRetry}>
              {t("list.retry")}
            </button>
          )}
        </div>
      )}
      {!isLoading && !loadError && items.length === 0 && <p className="list-note">{emptyText}</p>}
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
      {hasMore && onLoadMore && <LoadMore isLoading={isLoading} count={items.length} onLoadMore={onLoadMore} />}
    </section>
  );
}

/** How far below the screen the next messages start loading, so scrolling rarely has to wait. */
const LOAD_AHEAD = "600px";

interface LoadMoreProps {
  isLoading: boolean;
  /** Rows on screen: after each load the button is looked at again. */
  count: number;
  onLoadMore: () => void;
}

/**
 * The end of the list. Scrolling near it loads the next messages by itself; the button stays for
 * the keyboard, and for a browser without IntersectionObserver.
 */
function LoadMore({ isLoading, count, onLoadMore }: LoadMoreProps) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const node = ref.current;
    if (!node || isLoading || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          observer.disconnect();
          onLoadMore();
        }
      },
      { rootMargin: LOAD_AHEAD },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [isLoading, count, onLoadMore]);
  return (
    <button ref={ref} type="button" className="load-more" onClick={onLoadMore} disabled={isLoading}>
      {isLoading ? t("list.loading") : t("list.more")}
    </button>
  );
}
