import { useEffect, useState } from "react";
import { api, onEvent } from "../../api/client";
import type { ImportKind, ImportProgress, ImportSource, ImportTarget } from "../../api/types";
import { t, type MessageKey } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { formatBytes } from "../format";
import { mailAccounts } from "../folders";
import { useApp } from "../state/AppContext";
import { OriginalsList } from "./OriginalsList";

const KINDS: readonly { kind: ImportKind; label: MessageKey }[] = [
  { kind: "pst", label: "import.kind.pst" },
  { kind: "wlm", label: "import.kind.wlm" },
  { kind: "dbx", label: "import.kind.dbx" },
  { kind: "eml", label: "import.kind.eml" },
  { kind: "mbox", label: "import.kind.mbox" },
];

function progressText(p: ImportProgress): string {
  return p.total === null
    ? t("import.progressUnknown", { done: p.done })
    : t("import.progress", { done: p.done, total: p.total });
}

/** "Oude mail ophalen": choose a file, choose where it goes, watch it come in. */
export function ImportView() {
  const { accounts, showError, reload } = useApp();
  const mail = mailAccounts(accounts);
  const [sources, setSources] = useState<ImportSource[]>([]);
  const [source, setSource] = useState<ImportSource | null>(null);
  const [targetKind, setTargetKind] = useState<"local" | "mailbox">("local");
  const [accountId, setAccountId] = useState(mail[0]?.id ?? "");
  const [jobId, setJobId] = useState<string | null>(null);
  const [progressByJob, setProgressByJob] = useState<Readonly<Record<string, ImportProgress>>>({});
  const [doneCount, setDoneCount] = useState(0);

  useEffect(() => {
    api.findImportSources().then(setSources).catch(showError);
  }, [showError]);

  // Listen from the start, so no progress is missed while the import starts.
  useEffect(() => {
    let stop: (() => void) | null = null;
    let alive = true;
    onEvent("import:progress", (p) => {
      setProgressByJob((prev) => ({ ...prev, [p.jobId]: p }));
      if (p.finished) {
        setDoneCount((n) => n + 1);
        void reload();
      }
    })
      .then((u) => {
        if (alive) stop = u;
        else u();
      })
      .catch(showError);
    return () => {
      alive = false;
      stop?.();
    };
  }, [reload, showError]);

  const pickOther = async (kind: ImportKind) => {
    try {
      const picked = await api.pickImportFile(kind);
      if (picked) setSource(picked);
    } catch (e: unknown) {
      showError(e);
    }
  };

  const start = async () => {
    if (!source) return;
    const target: ImportTarget = targetKind === "mailbox" ? { kind: "mailbox", accountId } : { kind: "local" };
    try {
      setJobId(await api.startImport(source, target));
    } catch (e: unknown) {
      showError(e);
    }
  };

  const progress = jobId ? (progressByJob[jobId] ?? null) : null;
  const isRunning = progress !== null && !progress.finished;

  return (
    <section className="page import">
      <h2>{t("import.title")}</h2>

      <h3>{t("import.found")}</h3>
      {sources.length === 0 && <p>{t("import.none")}</p>}
      <ul className="plain-list">
        {sources.map((s) => (
          <li key={s.path} className="attachment-row">
            <span>{s.label}{s.size ? ` (${formatBytes(s.size)})` : ""}</span>
            <button type="button" aria-pressed={source?.path === s.path} onClick={() => setSource(s)}>
              {t("import.choose")}
            </button>
          </li>
        ))}
      </ul>

      <h3>{t("import.other")}</h3>
      <div className="button-row">
        {KINDS.map((k) => (
          <button key={k.kind} type="button" onClick={() => void pickOther(k.kind)}>{t(k.label)}</button>
        ))}
      </div>
      {source && <p className="calm-line"><strong>{t("import.chosen", { label: source.label })}</strong></p>}

      <fieldset>
        <legend>{t("import.where")}</legend>
        <label className="check-row">
          <input type="radio" name="import-target" checked={targetKind === "local"} onChange={() => setTargetKind("local")} />
          {t("import.local")}
        </label>
        {mail.length > 0 && (
          <label className="check-row">
            <input type="radio" name="import-target" checked={targetKind === "mailbox"} onChange={() => setTargetKind("mailbox")} />
            {t("import.mailbox")}
          </label>
        )}
        {targetKind === "mailbox" && mail.length > 1 && (
          <div className="form-row">
            <label htmlFor="import-account">{t("import.account")}</label>
            <select id="import-account" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
              {mail.map((a) => <option key={a.id} value={a.id}>{a.displayName}</option>)}
            </select>
          </div>
        )}
      </fieldset>

      <div className="button-row">
        <button type="button" className="primary" disabled={!source || isRunning} onClick={start}>
          {t("import.start")}
        </button>
      </div>

      {progress && (
        <div className="import-progress" role="status">
          <progress max={progress.total ?? undefined} value={progress.done} />
          <p>{progress.finished ? t("import.done", { done: progress.done }) : progressText(progress)}</p>
          {progress.currentFolder && !progress.finished && <p>{t("import.folder", { folder: progress.currentFolder })}</p>}
          {progress.error && <ErrorBox error={progress.error} />}
        </div>
      )}

      <OriginalsList refreshKey={doneCount} />
    </section>
  );
}
