import { useState } from "react";
import { api } from "../../api/client";
import { toVemError } from "../../api/errors";
import type { ServerSettings, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { useApp } from "../state/AppContext";

type Step = "form" | "looking" | "confirm" | "adding";

interface AccountSetupProps {
  isFirstRun: boolean;
}

/** "Postvak toevoegen": name, address, password. The rest is looked up. */
export function AccountSetup({ isFirstRun }: AccountSetupProps) {
  const { reload, go, setLocation } = useApp();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [step, setStep] = useState<Step>("form");
  const [found, setFound] = useState<ServerSettings | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [error, setError] = useState<VemError | null>(null);

  const add = async (settings: ServerSettings) => {
    setStep("adding");
    try {
      await api.addAccount(name.trim(), email.trim(), password, settings);
      setPassword("");
      await reload();
      const folders = await api.listFolders(null);
      const accounts = await api.listAccounts();
      const mail = accounts.filter((a) => a.kind === "imap");
      const inbox = folders.find((f) => f.role === "inbox" && f.accountId === mail[mail.length - 1]?.id);
      if (inbox) setLocation({ kind: "folder", folderId: inbox.id });
      go({ kind: "mail" });
    } catch (e: unknown) {
      setError(toVemError(e));
      setStep("form");
    }
  };

  const handleSubmit = async () => {
    setProblem(null);
    setError(null);
    if (name.trim() === "" || email.trim() === "" || password === "") {
      setProblem(t("setup.missing"));
      return;
    }
    setStep("looking");
    try {
      const settings = await api.lookupSettings(email.trim());
      setFound(settings);
      if (settings.needsConfirmation) {
        setStep("confirm");
        return;
      }
      await add(settings);
    } catch (e: unknown) {
      setError(toVemError(e));
      setStep("form");
    }
  };

  const isBusy = step === "looking" || step === "adding";

  return (
    <section className="page setup">
      {isFirstRun ? (
        <>
          <h2>{t("setup.welcome")}</h2>
          <p>{t("setup.welcomeText")}</p>
        </>
      ) : (
        <h2>{t("setup.title")}</h2>
      )}
      {error && <ErrorBox error={error} onClose={() => setError(null)} />}
      {problem && <p className="form-problem" role="alert">{problem}</p>}
      {step === "confirm" && found ? (
        <div className="confirm-host" role="alertdialog" aria-labelledby="confirm-host-title">
          <h3 id="confirm-host-title">{t("setup.confirmTitle")}</h3>
          <p>{t("setup.confirmText", { host: found.imapHost })}</p>
          <div className="button-row">
            <button type="button" className="primary" onClick={() => void add(found)}>{t("setup.confirmYes")}</button>
            <button type="button" onClick={() => setStep("form")}>{t("setup.confirmNo")}</button>
          </div>
        </div>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void handleSubmit();
          }}
        >
          <div className="form-row">
            <label htmlFor="setup-name">{t("setup.name")}</label>
            <input id="setup-name" autoComplete="name" value={name} onChange={(e) => setName(e.target.value)} />
            <span className="hint">{t("setup.nameHint")}</span>
          </div>
          <div className="form-row">
            <label htmlFor="setup-email">{t("setup.email")}</label>
            <input id="setup-email" type="email" autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} />
          </div>
          <div className="form-row">
            <label htmlFor="setup-password">{t("setup.password")}</label>
            <input id="setup-password" type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </div>
          <div className="button-row">
            <button type="submit" className="primary" disabled={isBusy}>{t("setup.next")}</button>
            {!isFirstRun && (
              <button type="button" onClick={() => go({ kind: "settings" })}>{t("setup.cancel")}</button>
            )}
          </div>
          {step === "looking" && <p role="status">{t("setup.looking")}</p>}
          {step === "adding" && <p role="status">{t("setup.adding")}</p>}
        </form>
      )}
    </section>
  );
}
