import type { ImportProgress, ImportSkip } from "../../api/types";
import { t } from "../../i18n";
import { CopyNoteText, ErrorBox } from "../common/ErrorBox";
import { useCopyNote } from "../common/useCopyNote";

interface ImportProgressViewProps {
  progress: ImportProgress;
}

function progressText(p: ImportProgress): string {
  return p.total === null
    ? t("import.progressUnknown", { done: p.done })
    : t("import.progress", { done: p.done, total: p.total });
}

function doneText(p: ImportProgress): string {
  return p.skipped.length > 0
    ? t("import.doneSkipped", { done: p.done, skipped: p.skipped.length })
    : t("import.done", { done: p.done });
}

/** The skipped folders as plain lines: folder and the core's reason, exactly as it gave it. */
export function skippedList(skipped: readonly ImportSkip[]): string {
  return [t("import.skippedTitle"), ...skipped.map((s) => `${s.folder}: ${s.reason}`)].join("\n");
}

/** The left-out folders: the reasons are technical, so they only show as details, like a server reply. */
function SkippedDetails({ skipped }: { skipped: readonly ImportSkip[] }) {
  const note = useCopyNote();
  const lines = skipped.map((s) => `${s.folder}: ${s.reason}`).join("\n");
  return (
    <div className="error-raw">
      <p className="error-label">{t("error.details")}</p>
      <pre className="error-reply">{lines}</pre>
      <div className="button-row">
        <button type="button" onClick={() => void note.copy(skippedList(skipped))}>
          {t("import.copyList")}
        </button>
        <CopyNoteText result={note.result} copiedText={t("import.listCopied")} />
      </div>
    </div>
  );
}

/** Progress while an import runs; when it is done, what came in, what was there already and what was left out. */
export function ImportProgressView({ progress }: ImportProgressViewProps) {
  if (!progress.finished) {
    return (
      <div className="import-progress" role="status">
        <progress max={progress.total ?? undefined} value={progress.done} />
        <p>{progressText(progress)}</p>
        {progress.currentFolder && <p>{t("import.folder", { folder: progress.currentFolder })}</p>}
      </div>
    );
  }
  return (
    <div className="import-progress">
      {progress.error ? (
        <ErrorBox error={progress.error} />
      ) : (
        <p role="status">
          <strong>{doneText(progress)}</strong>
        </p>
      )}
      {progress.alreadyThere > 0 && <p>{t("import.alreadyThere", { count: progress.alreadyThere })}</p>}
      {(progress.contactsAdded ?? 0) > 0 && <p>{t("import.contactsAdded", { count: progress.contactsAdded ?? 0 })}</p>}
      {progress.allMarkedRead && <p>{t("import.allMarkedRead")}</p>}
      {progress.skipped.length > 0 && <SkippedDetails skipped={progress.skipped} />}
    </div>
  );
}
