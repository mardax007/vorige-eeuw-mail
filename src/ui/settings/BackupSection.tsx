import { useEffect, useState } from "react";
import { api, onEvent } from "../../api/client";
import type { BackupInfo, BackupProgress } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { ErrorBox } from "../common/ErrorBox";
import { dateOnly, formatBytes } from "../format";
import { useApp } from "../state/AppContext";

function finishedText(p: BackupProgress): string {
  if (p.stopped) return p.restoring ? t("backup.restoreStopped") : t("backup.stopped");
  return p.restoring ? t("backup.restartSoon") : t("backup.done", { folder: p.folder ?? "" });
}

/**
 * "Reservekopie" in Instellingen, on the PC only (C14, decision 29): everything but passwords to
 * a USB stick, and back. Putting one back replaces everything, so it asks once first.
 */
export function BackupSection() {
  const { showError } = useApp();
  const [progress, setProgress] = useState<BackupProgress | null>(null);
  const [candidate, setCandidate] = useState<BackupInfo | null>(null);
  const isRunning = progress !== null && !progress.finished;

  useEffect(() => {
    let stop: (() => void) | undefined;
    let isGone = false;
    void onEvent("backup:progress", setProgress).then((u) => (isGone ? u() : (stop = u)));
    return () => {
      isGone = true;
      stop?.();
    };
  }, []);

  const make = async () => {
    try {
      if (await api.startBackup()) setProgress({ restoring: false, doneBytes: 0, totalBytes: 0, finished: false, stopped: false });
    } catch (e: unknown) {
      showError(e);
    }
  };

  const pick = async () => {
    try {
      setCandidate(await api.pickBackup());
    } catch (e: unknown) {
      showError(e);
    }
  };

  const restore = async (info: BackupInfo) => {
    setCandidate(null);
    try {
      await api.startRestore(info.path);
      setProgress({ restoring: true, doneBytes: 0, totalBytes: info.bytes, finished: false, stopped: false });
    } catch (e: unknown) {
      showError(e);
    }
  };

  const confirmText = candidate
    ? t("backup.confirmRestore", {
        date: dateOnly(candidate.createdAt),
        count: candidate.messages,
        accounts: candidate.accounts.map((a) => a.email).join(", "),
      })
    : "";

  return (
    <>
      <h3>{t("backup.title")}</h3>
      <p>{t("backup.intro")}</p>
      <div className="button-row">
        <button type="button" disabled={isRunning} onClick={() => void make()}>
          {t("backup.make")}
        </button>
        <button type="button" disabled={isRunning} onClick={() => void pick()}>
          {t("backup.restore")}
        </button>
      </div>
      {isRunning && progress && (
        <div className="import-progress" role="status">
          <progress aria-label={t("backup.progressLabel")} max={progress.totalBytes || undefined} value={progress.doneBytes} />
          <p>
            {t(progress.restoring ? "backup.restoring" : "backup.making", {
              done: formatBytes(progress.doneBytes),
              total: formatBytes(progress.totalBytes),
            })}
          </p>
          <div className="button-row">
            <button type="button" onClick={() => void api.stopBackup().catch(showError)}>
              {t("backup.stop")}
            </button>
          </div>
        </div>
      )}
      {progress?.finished &&
        (progress.error ? (
          <ErrorBox error={progress.error} />
        ) : (
          <p className="calm-line" role="status">
            {finishedText(progress)}
          </p>
        ))}
      {candidate && (
        <ConfirmDialog
          text={confirmText}
          confirmLabel={t("backup.confirmButton")}
          onConfirm={() => void restore(candidate)}
          onCancel={() => setCandidate(null)}
        />
      )}
    </>
  );
}
