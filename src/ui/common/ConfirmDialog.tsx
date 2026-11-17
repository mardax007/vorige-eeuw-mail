import { useEffect, useRef } from "react";
import { t } from "../../i18n";
import { useBackStep } from "./useBackStep";

interface ConfirmDialogProps {
  text: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Asked only for what cannot be undone (docs/screens.md, "Mistakes"). Focus starts on Annuleren,
 * stays inside the dialog while it is open, and goes back to the button that opened it. Escape
 * and Android's back key cancel.
 */
export function ConfirmDialog({ text, confirmLabel, onConfirm, onCancel }: ConfirmDialogProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const onCancelRef = useRef(onCancel);
  useEffect(() => {
    onCancelRef.current = onCancel;
  });
  useBackStep(true, () => onCancelRef.current(), 2);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    cancelRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancelRef.current();
        return;
      }
      if (e.key !== "Tab" || !dialogRef.current) return;
      const buttons = Array.from(dialogRef.current.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
      if (buttons.length === 0) return;
      const first = buttons[0];
      const last = buttons[buttons.length - 1];
      const inside = dialogRef.current.contains(document.activeElement);
      if (e.shiftKey && (document.activeElement === first || !inside)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (document.activeElement === last || !inside)) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      if (opener?.isConnected) opener.focus();
    };
  }, []);

  return (
    <div className="dialog-backdrop">
      <div ref={dialogRef} className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-text">
        <p id="confirm-text">{text}</p>
        <div className="button-row">
          <button type="button" className="danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
          <button type="button" ref={cancelRef} onClick={onCancel}>
            {t("general.cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}
