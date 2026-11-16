import { useCallback } from "react";
import { api } from "../../api/client";
import type { MessageDetail } from "../../api/types";
import { t } from "../../i18n";
import { prefillDraft, type ReplyKind } from "../compose/prefill";
import { useApp } from "../state/AppContext";

export interface MessageActions {
  remove: (ids: string[]) => Promise<boolean>;
  move: (ids: string[], folderId: string) => Promise<boolean>;
  respond: (message: MessageDetail, kind: ReplyKind) => void;
}

/** Verwijderen, Verplaatsen and the reply buttons, shared by the buttons and the keyboard. */
export function useMessageActions(): MessageActions {
  const { accounts, compose, newDraft, showUndo, showError, reload } = useApp();

  const undoWith = useCallback(
    (token: string) => async () => {
      await api.undo(token);
      await reload();
    },
    [reload],
  );

  const remove = useCallback(
    async (ids: string[]) => {
      try {
        const result = await api.deleteMessages(ids);
        const text = result.count === 1 ? t("undo.deletedOne") : t("undo.deletedMany", { count: result.count });
        showUndo(text, undoWith(result.token));
        await reload();
        return true;
      } catch (e: unknown) {
        showError(e);
        return false;
      }
    },
    [reload, showError, showUndo, undoWith],
  );

  const move = useCallback(
    async (ids: string[], folderId: string) => {
      try {
        const result = await api.moveMessages(ids, folderId);
        const text = result.count === 1 ? t("undo.movedOne") : t("undo.movedMany", { count: result.count });
        showUndo(text, undoWith(result.token));
        await reload();
        return true;
      } catch (e: unknown) {
        showError(e);
        return false;
      }
    },
    [reload, showError, showUndo, undoWith],
  );

  const respond = useCallback(
    (message: MessageDetail, kind: ReplyKind) => {
      // Mail kept on this computer cannot be answered from itself; use the first real mailbox.
      const own = accounts.find((a) => a.id === message.accountId && a.kind === "imap") ?? accounts.find((a) => a.kind === "imap");
      const base = newDraft({ accountId: own?.id ?? "", bodyText: "" });
      compose(prefillDraft(base, message, kind, own?.email ?? "", own?.signature));
    },
    [accounts, compose, newDraft],
  );

  return { remove, move, respond };
}
