import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { api, onEvent, type Unlisten } from "../../api/client";
import type { Addr, OutboxItem, VemError } from "../../api/types";
import { parseAddressList } from "../compose/address";

interface CoreEventHandlers {
  reload: () => Promise<void>;
  setOutbox: Dispatch<SetStateAction<OutboxItem[]>>;
  setSyncErrors: Dispatch<SetStateAction<Record<string, VemError>>>;
  setSyncBusy: Dispatch<SetStateAction<Record<string, boolean>>>;
  showMessage: (folderId: string, messageId: string) => void;
  composeTo: (to: Addr[]) => void;
  showError: (e: unknown) => void;
}

function withoutKey<T>(record: Readonly<Record<string, T>>, key: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

/** Listens to the core's events for as long as the app is open. */
export function useCoreEvents(handlers: CoreEventHandlers): void {
  const ref = useRef(handlers);
  useEffect(() => {
    ref.current = handlers;
  });

  useEffect(() => {
    let stopped = false;
    const unlisten: Unlisten[] = [];
    const add = (p: Promise<Unlisten>) =>
      p.then((u) => (stopped ? u() : unlisten.push(u))).catch((e: unknown) => ref.current.showError(e));

    add(onEvent("mail:changed", () => void ref.current.reload()));
    add(onEvent("outbox:changed", (items) => ref.current.setOutbox(items)));
    add(
      onEvent("sync:state", ({ accountId, busy, error }) => {
        ref.current.setSyncBusy((prev) => (busy ? { ...prev, [accountId]: true } : withoutKey(prev, accountId)));
        ref.current.setSyncErrors((prev) =>
          error ? { ...prev, [accountId]: error } : withoutKey(prev, accountId),
        );
      }),
    );
    add(
      onEvent("open:message", ({ messageId }) => {
        api
          .getMessage(messageId, false)
          .then((m) => ref.current.showMessage(m.folderId, m.id))
          .catch((e: unknown) => ref.current.showError(e));
      }),
    );
    add(onEvent("open:compose", ({ to }) => ref.current.composeTo(parseAddressList(to).addresses)));

    return () => {
      stopped = true;
      unlisten.forEach((u) => u());
    };
  }, []);
}
