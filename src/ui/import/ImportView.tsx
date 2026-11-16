import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { Account, ImportKind, ImportSource, ImportTarget } from "../../api/types";
import { t, type MessageKey } from "../../i18n";
import { formatBytes } from "../format";
import { mailAccounts } from "../folders";
import { useApp } from "../state/AppContext";
import { ContactsFromLiveMail } from "./ContactsFromLiveMail";
import { ImportProgressView } from "./ImportProgressView";
import { OriginalsList } from "./OriginalsList";
import { useImportProgress } from "./useImportProgress";

const KINDS: readonly { kind: ImportKind; label: MessageKey }[] = [
  { kind: "pst", label: "import.kind.pst" },
  { kind: "wlm", label: "import.kind.wlm" },
  { kind: "dbx", label: "import.kind.dbx" },
  { kind: "eml", label: "import.kind.eml" },
  { kind: "mbox", label: "import.kind.mbox" },
];

/** Where the old mail comes from: a file or folder on this computer, or an added mailbox. */
type ImportChoice = { kind: "file"; source: ImportSource } | { kind: "account"; accountId: string };

function chosenText(choice: ImportChoice, accounts: readonly Account[]): string {
  if (choice.kind === "file") return t("import.chosen", { label: choice.source.label });
  const account = accounts.find((a) => a.id === choice.accountId);
  return t("import.chosenAccount", { account: account?.displayName ?? "" });
}

interface AccountSourcesProps {
  mail: readonly Account[];
  choice: ImportChoice | null;
  onChoose: (accountId: string) => void;
  onAddAccount: () => void;
}

/** "Uit een ander postvak": the added mailboxes, or how to add the old one first. */
function AccountSources({ mail, choice, onChoose, onAddAccount }: AccountSourcesProps) {
  return (
    <>
      <h3>{t("import.fromAccount")}</h3>
      <p>{t("import.fromAccountText")}</p>
      {mail.length < 2 ? (
        <div className="button-row">
          <p className="calm-line">{t("import.fromAccountNeedsTwo")}</p>
          <button type="button" onClick={onAddAccount}>{t("import.addAccount")}</button>
        </div>
      ) : (
        <ul className="plain-list">
          {mail.map((a) => (
            <li key={a.id} className="attachment-row">
              <span>
                {a.displayName} <span className="row-address">&lt;{a.email}&gt;</span>
              </span>
              <button
                type="button"
                aria-pressed={choice?.kind === "account" && choice.accountId === a.id}
                onClick={() => onChoose(a.id)}
              >
                {t("import.choose")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

/** "Oude mail ophalen": choose a file, folder or old mailbox, choose where it goes, watch it come in. */
export function ImportView() {
  const { accounts, showError, reload, go } = useApp();
  const mail = mailAccounts(accounts);
  const [sources, setSources] = useState<ImportSource[]>([]);
  const [choice, setChoice] = useState<ImportChoice | null>(null);
  const [targetKind, setTargetKind] = useState<"local" | "mailbox">("local");
  const [accountId, setAccountId] = useState(mail[0]?.id ?? "");
  const [isStarting, setIsStarting] = useState(false);
  const [doneCount, setDoneCount] = useState(0);
  const job = useImportProgress(() => {
    setDoneCount((n) => n + 1);
    void reload();
  }, showError);

  useEffect(() => {
    api.findImportSources().then(setSources).catch(showError);
  }, [showError]);

  // The old mailbox itself is never offered as the place to put its mail.
  const sourceAccountId = choice?.kind === "account" ? choice.accountId : null;
  const targets = mail.filter((a) => a.id !== sourceAccountId);
  const targetId = targets.some((a) => a.id === accountId) ? accountId : (targets[0]?.id ?? "");
  const effectiveKind = targets.length === 0 ? "local" : targetKind;

  const pickOther = async (kind: ImportKind) => {
    try {
      const picked = await api.pickImportFile(kind);
      if (picked) setChoice({ kind: "file", source: picked });
    } catch (e: unknown) {
      showError(e);
    }
  };

  const start = async () => {
    if (!choice || job.isRunning || isStarting) return;
    const target: ImportTarget = effectiveKind === "mailbox" ? { kind: "mailbox", accountId: targetId } : { kind: "local" };
    setIsStarting(true);
    try {
      const jobId =
        choice.kind === "file"
          ? await api.startImport(choice.source, target)
          : await api.importFromAccount(choice.accountId, target);
      job.begin(jobId);
    } catch (e: unknown) {
      showError(e);
      // Perhaps an import is already running (started elsewhere): show it.
      job.refresh();
    } finally {
      setIsStarting(false);
    }
  };

  return (
    <section className="page import">
      <h2>{t("import.title")}</h2>

      <h3>{t("import.found")}</h3>
      {sources.length === 0 && <p>{t("import.none")}</p>}
      <ul className="plain-list">
        {sources.map((s) => (
          <li key={s.path} className="attachment-row">
            <span>{s.label}{s.size ? ` (${formatBytes(s.size)})` : ""}</span>
            <button
              type="button"
              aria-pressed={choice?.kind === "file" && choice.source.path === s.path}
              onClick={() => setChoice({ kind: "file", source: s })}
            >
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

      <AccountSources
        mail={mail}
        choice={choice}
        onChoose={(id) => setChoice({ kind: "account", accountId: id })}
        onAddAccount={() => go({ kind: "setup" })}
      />

      {choice && <p className="calm-line"><strong>{chosenText(choice, accounts)}</strong></p>}

      <fieldset>
        <legend>{t("import.where")}</legend>
        <label className="check-row">
          <input type="radio" name="import-target" checked={effectiveKind === "local"} onChange={() => setTargetKind("local")} />
          {t("import.local")}
        </label>
        {targets.length > 0 && (
          <label className="check-row">
            <input type="radio" name="import-target" checked={effectiveKind === "mailbox"} onChange={() => setTargetKind("mailbox")} />
            {targets.length === 1 && sourceAccountId !== null
              ? t("import.mailboxNamed", { account: targets[0].displayName })
              : t("import.mailbox")}
          </label>
        )}
        {effectiveKind === "mailbox" && targets.length > 1 && (
          <div className="form-row">
            <label htmlFor="import-account">{t("import.account")}</label>
            <select id="import-account" value={targetId} onChange={(e) => setAccountId(e.target.value)}>
              {targets.map((a) => <option key={a.id} value={a.id}>{a.displayName}</option>)}
            </select>
          </div>
        )}
      </fieldset>

      <div className="button-row">
        <button type="button" className="primary" disabled={!choice || job.isRunning || isStarting} onClick={start}>
          {t("import.start")}
        </button>
      </div>

      {job.progress && <ImportProgressView progress={job.progress} />}

      <ContactsFromLiveMail />

      <OriginalsList refreshKey={doneCount} />
    </section>
  );
}
