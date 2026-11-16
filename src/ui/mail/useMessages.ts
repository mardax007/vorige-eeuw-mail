import { useCallback, useEffect, useState } from "react";
import { api } from "../../api/client";
import type { MessageSummary } from "../../api/types";
import type { MailLocation } from "../state/AppContext";

export const PAGE_SIZE = 50;

interface MessagesState {
  messages: MessageSummary[];
  isLoading: boolean;
  hasMore: boolean;
  loadMore: () => void;
}

async function load(location: MailLocation, limit: number): Promise<MessageSummary[]> {
  if (location.kind === "folder") return api.listMessages(location.folderId, 0, limit);
  if (location.kind === "tag") return api.listTagMessages(location.tagId);
  return [];
}

/** Messages at one place, loaded again whenever mail changed. */
export function useMessages(
  location: MailLocation,
  mailVersion: number,
  onError: (e: unknown) => void,
): MessagesState {
  const [messages, setMessages] = useState<MessageSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [limit, setLimit] = useState(PAGE_SIZE);
  const key = location.kind === "folder" ? location.folderId : location.kind === "tag" ? `tag:${location.tagId}` : "outbox";

  useEffect(() => {
    setLimit(PAGE_SIZE);
  }, [key]);

  useEffect(() => {
    let alive = true;
    setIsLoading(true);
    load(location, limit)
      .then((list) => {
        if (alive) setMessages(list);
      })
      .catch((e: unknown) => {
        if (alive) onError(e);
      })
      .finally(() => {
        if (alive) setIsLoading(false);
      });
    return () => {
      alive = false;
    };
    // location is identified by key; reloading on every new object would loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, limit, mailVersion, onError]);

  const loadMore = useCallback(() => setLimit((l) => l + PAGE_SIZE), []);
  const hasMore = location.kind === "folder" && messages.length >= limit;

  return { messages, isLoading, hasMore, loadMore };
}
