import { useCallback, useRef } from "react";
import { api } from "../../api/client";
import type { MessageDetail } from "../../api/types";
import { t } from "../../i18n";
import { prefillDraft, type ReplyKind } from "../compose/prefill";
import { folderLabel } from "../folders";
import { useApp } from "../state/AppContext";

export interface MessageActions {
  remove: (ids: string[]) => Promise<boolean>;
  move: (ids: string[], folderId: string) => Promise<boolean>;
  respond: (message: MessageDetail, kind: ReplyKind) => void;
}

/** Verwijderen, Verplaatsen and the reply buttons, shared by the buttons and the keyboard. */
export function useMessageActions(): MessageActions {
  const { accounts, folders, compose, newDraft, showUndo, showError, reload } = useApp();
  const forwarding = useRef(false);

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
        // Nothing moved (already in Verwijderde items): no bar that says otherwise.
        if (result.count === 0) return false;
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
        if (result.count === 0) return false;
        const target = folders.find((f) => f.id === folderId);
        const folder = target ? folderLabel(target) : "";
        const text =
          result.count === 1 ? t("undo.movedOne", { folder }) : t("undo.movedMany", { count: result.count, folder });
        showUndo(text, undoWith(result.token));
        await reload();
        return true;
      } catch (e: unknown) {
        showError(e);
        return false;
      }
    },
    [folders, reload, showError, showUndo, undoWith],
  );

  const respond = useCallback(
    (message: MessageDetail, kind: ReplyKind) => {
      // Mail kept on this computer cannot be answered from itself; use the first real mailbox.
      const own = accounts.find((a) => a.id === message.accountId && a.kind === "imap") ?? accounts.find((a) => a.kind === "imap");
      const base = newDraft({ accountId: own?.id ?? "", bodyText: "" });
      const draft = prefillDraft(base, message, kind, own?.email ?? "", own?.signature);
      if (kind !== "forward" || message.attachments.length === 0) {
        compose(draft);
        return;
      }
      // Doorsturen sends the attachments along, as Outlook does. If they cannot be fetched, Write
      // does not open: a forward without the photos would look complete and go out as such.
      // A second tap while the files are being written starts nothing.
      if (forwarding.current) return;
      forwarding.current = true;
      api
        .forwardAttachments(message.id)
        .then((attachments) => compose({ ...draft, attachments }))
        .catch((e: unknown) => showError(e))
        .finally(() => {
          forwarding.current = false;
        });
    },
    [accounts, compose, newDraft, showError],
  );

  return { remove, move, respond };
}
