import { useEffect, useRef } from "react";
import { t } from "../../i18n";

interface ConfirmDialogProps {
  text: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Asked only for what cannot be undone (docs/screens.md, "Mistakes"). */
export function ConfirmDialog({ text, confirmLabel, onConfirm, onCancel }: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="dialog-backdrop">
      <div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-text">
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
