import { useCallback, useEffect, useState } from "react";
import { api } from "../../api/client";
import type { OriginalCopy } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { dateOnly, formatBytes } from "../format";
import { useApp } from "../state/AppContext";

interface OriginalsListProps {
  /** Goes up after an import, so a new copy shows. */
  refreshKey: number;
}

/** "Originele bestanden": the kept copies of imported files, each with Verwijderen. */
export function OriginalsList({ refreshKey }: OriginalsListProps) {
  const { showError } = useApp();
  const [originals, setOriginals] = useState<OriginalCopy[]>([]);
  const [toDelete, setToDelete] = useState<OriginalCopy | null>(null);

  const load = useCallback(async () => {
    try {
      setOriginals(await api.listOriginals());
    } catch (e: unknown) {
      showError(e);
    }
  }, [showError]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  const remove = async (copy: OriginalCopy) => {
    setToDelete(null);
    try {
      await api.deleteOriginal(copy.id);
      await load();
    } catch (e: unknown) {
      showError(e);
    }
  };

  return (
    <div className="originals">
      <h3>{t("import.originals")}</h3>
      <p>{t("import.originalsText")}</p>
      {originals.length === 0 && <p>{t("import.noOriginals")}</p>}
      <ul className="plain-list">
        {originals.map((o) => (
          <li key={o.id} className="attachment-row">
            <span>
              {o.label} ({formatBytes(o.size)}), {t("import.importedAt", { date: dateOnly(o.importedAt) })}
            </span>
            <button type="button" onClick={() => setToDelete(o)}>{t("import.delete")}</button>
          </li>
        ))}
      </ul>
      {toDelete && (
        <ConfirmDialog
          text={t("import.deleteConfirm", { label: toDelete.label })}
          confirmLabel={t("import.deleteYes")}
          onConfirm={() => void remove(toDelete)}
          onCancel={() => setToDelete(null)}
        />
      )}
    </div>
  );
}
