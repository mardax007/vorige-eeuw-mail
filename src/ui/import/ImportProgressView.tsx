import type { ImportProgress, ImportSkip } from "../../api/types";
import { t } from "../../i18n";
import { CopyNoteText, ErrorBox } from "../common/ErrorBox";
import { useCopyNote } from "../common/useCopyNote";

interface ImportProgressViewProps {
  progress: ImportProgress;
  /** Opens the folder the new mail went into. */
  onOpen: (folderId: string) => void;
}

/** Which file or mailbox the progress or result is about, so an old result never reads as a new one. */
function SourceLine({ source }: { source?: string | null }) {
  return source ? <p className="calm-line">{t("import.resultFor", { source })}</p> : null;
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
export function ImportProgressView({ progress, onOpen }: ImportProgressViewProps) {
  if (!progress.finished) {
    return (
      <div className="import-progress" role="status">
        <SourceLine source={progress.source} />
        <progress max={progress.total ?? undefined} value={progress.done} />
        <p>{progressText(progress)}</p>
        {progress.currentFolder && <p>{t("import.folder", { folder: progress.currentFolder })}</p>}
      </div>
    );
  }
  return (
    <div className="import-progress">
      <SourceLine source={progress.source} />
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
      {progress.rescued && !progress.error && <p>{t("import.rescued")}</p>}
      {progress.folderId && (
        <div className="button-row">
          <button type="button" onClick={() => onOpen(progress.folderId ?? "")}>{t("import.open")}</button>
        </div>
      )}
      {progress.skipped.length > 0 && <SkippedDetails skipped={progress.skipped} />}
    </div>
  );
}
