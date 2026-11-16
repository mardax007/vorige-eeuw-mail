import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import type { MessageSummary } from "../../api/types";
import type { MailLocation } from "../state/AppContext";

export const PAGE_SIZE = 50;

interface MessagesState {
  messages: MessageSummary[];
  isLoading: boolean;
  hasMore: boolean;
  loadMore: () => void;
  /** True when the last load failed; the error itself is shown at the top. */
  loadError: boolean;
  retry: () => void;
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
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const key = location.kind === "folder" ? location.folderId : location.kind === "tag" ? `tag:${location.tagId}` : "outbox";
  // The place the list on screen belongs to: a failed reload there keeps it, another place does not.
  const loadedKey = useRef<string | null>(null);

  useEffect(() => {
    setLimit(PAGE_SIZE);
  }, [key]);

  useEffect(() => {
    let alive = true;
    setIsLoading(true);
    load(location, limit)
      .then((list) => {
        if (!alive) return;
        loadedKey.current = key;
        setMessages(list);
        setLoadError(false);
      })
      .catch((e: unknown) => {
        if (!alive) return;
        // Only the first load of a place empties the list; a failed reload keeps what is there.
        if (loadedKey.current !== key) setMessages([]);
        setLoadError(true);
        onError(e);
      })
      .finally(() => {
        if (alive) setIsLoading(false);
      });
    return () => {
      alive = false;
    };
    // location is identified by key; reloading on every new object would loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, limit, mailVersion, attempt, onError]);

  const loadMore = useCallback(() => setLimit((l) => l + PAGE_SIZE), []);
  const retry = useCallback(() => setAttempt((n) => n + 1), []);
  const hasMore = !loadError && location.kind === "folder" && messages.length >= limit;

  return { messages, isLoading, hasMore, loadMore, loadError, retry };
}
