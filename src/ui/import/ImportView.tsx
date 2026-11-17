import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { toVemError } from "../../api/errors";
import type { Account, ImportKind, ImportSource, ImportTarget } from "../../api/types";
import { errorText, t, type MessageKey } from "../../i18n";
import { formatBytes } from "../format";
import { mailAccounts } from "../folders";
import { useApp } from "../state/AppContext";
import { ContactsFromLiveMail } from "./ContactsFromLiveMail";
import { ImportProgressView } from "./ImportProgressView";
import { OriginalsList } from "./OriginalsList";
import { PstPassword } from "./PstPassword";
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

const WRONG_PASSWORD = "VEM-IMPORT-READ-05";
/** The normal reader could not read the file: the rescue tool may still. */
const UNREADABLE = "VEM-IMPORT-READ-01";

/** "Oude mail ophalen": choose a file, folder or old mailbox, choose where it goes, watch it come in. */
export function ImportView() {
  const { accounts, showError, reload, go, appInfo } = useApp();
  // Decision 26: files from Outlook and Windows Live Mail are brought in on the PC only.
  const isPhone = appInfo?.isPhone === true;
  const mail = mailAccounts(accounts);
  // null while the computer is still being searched.
  const [sources, setSources] = useState<ImportSource[] | null>(null);
  const [choice, setChoiceState] = useState<ImportChoice | null>(null);
  const [password, setPassword] = useState("");
  const [passwordProblem, setPasswordProblem] = useState<string | null>(null);
  const [targetKind, setTargetKind] = useState<"local" | "mailbox">("local");
  const [accountId, setAccountId] = useState(mail[0]?.id ?? "");
  const [isStarting, setIsStarting] = useState(false);
  const [canRescue, setCanRescue] = useState(false);
  const [rescueTried, setRescueTried] = useState(false);
  const [doneCount, setDoneCount] = useState(0);
  const job = useImportProgress(() => {
    setDoneCount((n) => n + 1);
    void reload();
  }, showError);

  const setChoice = (next: ImportChoice) => {
    setChoiceState(next);
    setPassword("");
    setPasswordProblem(null);
    setRescueTried(false);
  };
  const needsPassword = choice?.kind === "file" && choice.source.protected === true;

  useEffect(() => {
    if (isPhone) return;
    api
      .findImportSources()
      .then(setSources)
      .catch((e: unknown) => {
        setSources([]);
        showError(e);
      });
    api.rescueAvailable().then(setCanRescue).catch(() => setCanRescue(false));
  }, [showError, isPhone]);

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

  const start = async (rescue = false) => {
    if (!choice || job.isRunning || isStarting) return;
    const target: ImportTarget = effectiveKind === "mailbox" ? { kind: "mailbox", accountId: targetId } : { kind: "local" };
    setIsStarting(true);
    if (rescue) setRescueTried(true);
    try {
      const jobId =
        choice.kind === "file"
          ? await api.startImport(choice.source, target, needsPassword ? password : null, rescue)
          : await api.importFromAccount(choice.accountId, target);
      setPasswordProblem(null);
      job.begin(jobId);
    } catch (e: unknown) {
      const error = toVemError(e);
      if (error.code === WRONG_PASSWORD) {
        const text = errorText(error.code);
        setPasswordProblem(`${text.sentence} ${text.next} (${error.code})`);
        return;
      }
      showError(e);
      // Perhaps an import is already running (started elsewhere): show it.
      job.refresh();
    } finally {
      setIsStarting(false);
    }
  };

  const offerRescue =
    canRescue &&
    !job.isRunning &&
    choice?.kind === "file" &&
    choice.source.kind === "pst" &&
    job.progress?.finished === true &&
    job.progress.error?.code === UNREADABLE &&
    !rescueTried;

  return (
    <section className="page import">
      <h2>{t("import.title")}</h2>

      {isPhone ? (
        <p>{t("import.phoneFiles")}</p>
      ) : (
        <>
          <h3>{t("import.found")}</h3>
          {sources === null && <p role="status">{t("import.searching")}</p>}
          {sources?.length === 0 && <p>{t("import.none")}</p>}
          <ul className="plain-list">
            {(sources ?? []).map((s) => (
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
        </>
      )}

      <AccountSources
        mail={mail}
        choice={choice}
        onChoose={(id) => setChoice({ kind: "account", accountId: id })}
        onAddAccount={() => go({ kind: "setup" })}
      />

      {choice && <p className="calm-line"><strong>{chosenText(choice, accounts)}</strong></p>}
      {needsPassword && (
        <PstPassword
          value={password}
          onChange={(v) => {
            setPassword(v);
            setPasswordProblem(null);
          }}
          problem={passwordProblem}
        />
      )}

      <fieldset>
        <legend>{t("import.where")}</legend>
        <label className="check-row">
          <input type="radio" name="import-target" checked={effectiveKind === "local"} onChange={() => setTargetKind("local")} />
          {isPhone ? t("import.localPhone") : t("import.local")}
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
        <button type="button" className="primary" disabled={!choice || job.isRunning || isStarting || (needsPassword && password === "")}
          onClick={() => void start()}
        >
          {t("import.start")}
        </button>
        {!choice && <span className="hint">{t("import.chooseFirst")}</span>}
      </div>

      {job.progress && <ImportProgressView progress={job.progress} />}
      {offerRescue && (
        <div className="button-row">
          <p className="calm-line">{t("import.rescue.text")}</p>
          <button type="button" disabled={isStarting} onClick={() => void start(true)}>
            {t("import.rescue.button")}
          </button>
        </div>
      )}

      {!isPhone && (
        <>
          <ContactsFromLiveMail />
          <OriginalsList refreshKey={doneCount} />
        </>
      )}
    </section>
  );
}
