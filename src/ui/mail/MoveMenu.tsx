import type { Folder } from "../../api/types";
import { t } from "../../i18n";
import { folderLabel } from "../folders";

interface MoveMenuProps {
  folders: readonly Folder[];
  currentFolderId: string;
  onMove: (folderId: string) => void;
  onClose: () => void;
}

/** The folders of the same mailbox, one button each. */
export function MoveMenu({ folders, currentFolderId, onMove, onClose }: MoveMenuProps) {
  const targets = folders.filter((f) => f.id !== currentFolderId);
  return (
    <div className="popup-panel" role="group" aria-label={t("read.moveTo")}>
      <h3>{t("read.moveTo")}</h3>
      <ul className="choice-list">
        {targets.map((f) => (
          <li key={f.id}>
            <button type="button" onClick={() => onMove(f.id)}>
              {folderLabel(f)}
            </button>
          </li>
        ))}
      </ul>
      <button type="button" onClick={onClose}>
        {t("general.cancel")}
      </button>
    </div>
  );
}
