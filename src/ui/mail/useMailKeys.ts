import { useEffect, useRef } from "react";
import { api } from "../../api/client";
import { useApp } from "../state/AppContext";
import { useMessageActions } from "./useMessageActions";

/** True when the key goes to a text field, where Delete and Ctrl+R mean something else. */
export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
}

/** Outlook's Delete and Ctrl+R, as extras next to the buttons. */
export function useMailKeys(openMessageId: string | null): void {
  const { setOpenMessageId, showError } = useApp();
  const actions = useMessageActions();
  const ref = useRef({ openMessageId, actions, setOpenMessageId, showError });

  useEffect(() => {
    ref.current = { openMessageId, actions, setOpenMessageId, showError };
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const { openMessageId: id, actions: act, setOpenMessageId: setOpen, showError: fail } = ref.current;
      if (id === null || isTyping(e.target)) return;
      if (e.key === "Delete") {
        e.preventDefault();
        void act.remove([id]).then((ok) => ok && setOpen(null));
      } else if (e.ctrlKey && (e.key === "r" || e.key === "R")) {
        e.preventDefault();
        api
          .getMessage(id, false)
          .then((m) => act.respond(m, "reply"))
          .catch((err: unknown) => fail(err));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
