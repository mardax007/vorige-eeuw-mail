import { useRef, useState } from "react";
import { api } from "../../api/client";
import { isStoppedSignIn, toVemError } from "../../api/errors";
import type { Account, ServerSettings, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { oauthDonePage } from "../settings/signIn";
import { useApp } from "../state/AppContext";
import { CredentialsStep, type SignInRoute } from "./CredentialsStep";

/** address → (confirm a guessed host) → credentials → adding or waiting for the browser. */
type Step = "address" | "looking" | "confirm" | "credentials" | "adding" | "waiting";

interface AccountSetupProps {
  isFirstRun: boolean;
}

/** "Postvak toevoegen": name and address, then a password or the Google/Microsoft sign-in. */
export function AccountSetup({ isFirstRun }: AccountSetupProps) {
  const { reload, go, setLocation, appInfo } = useApp();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [step, setStep] = useState<Step>("address");
  const [found, setFound] = useState<ServerSettings | null>(null);
  const [route, setRoute] = useState<SignInRoute>("password");
  const [problem, setProblem] = useState<string | null>(null);
  const [error, setError] = useState<VemError | null>(null);
  const isStopping = useRef(false);

  const finish = async (account: Account) => {
    await reload();
    const folders = await api.listFolders(account.id);
    const inbox = folders.find((f) => f.role === "inbox");
    if (inbox) setLocation({ kind: "folder", folderId: inbox.id });
    go({ kind: "mail" });
  };

  const run = async (busy: Step, add: () => Promise<Account>) => {
    setError(null);
    isStopping.current = false;
    setStep(busy);
    try {
      await finish(await add());
    } catch (e: unknown) {
      const err = toVemError(e);
      // Stoppen pressed: just go back to the choice, without an error.
      if (!isStopping.current && !isStoppedSignIn(err)) setError(err);
      setStep("credentials");
    }
  };

  const stopSignIn = async () => {
    isStopping.current = true;
    try {
      await api.cancelSignIn();
    } catch (e: unknown) {
      setError(toVemError(e));
    }
  };

  /** Chooses the sign-in way for the settings that were found. */
  const toCredentials = async (settings: ServerSettings) => {
    let next: SignInRoute = "password";
    if (settings.signIn !== "password") {
      try {
        next = (await api.oauthAvailable(settings.signIn)) ? "oauth" : "password";
      } catch (e: unknown) {
        setError(toVemError(e));
      }
    }
    setRoute(next);
    setStep("credentials");
  };

  const handleLookup = async () => {
    setProblem(null);
    setError(null);
    if (name.trim() === "" || email.trim() === "") {
      setProblem(t("setup.missingNameEmail"));
      return;
    }
    setStep("looking");
    try {
      const settings = await api.lookupSettings(email.trim());
      setFound(settings);
      if (settings.needsConfirmation) setStep("confirm");
      else await toCredentials(settings);
    } catch (e: unknown) {
      setError(toVemError(e));
      setStep("address");
    }
  };

  const back = () => {
    setError(null);
    setStep("address");
  };

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

      {(step === "address" || step === "looking") && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void handleLookup();
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
          <div className="button-row">
            <button type="submit" className="primary" disabled={step === "looking"}>{t("setup.next")}</button>
            {!isFirstRun && (
              <button type="button" onClick={() => go({ kind: "settings" })}>{t("setup.cancel")}</button>
            )}
          </div>
          {step === "looking" && <p role="status">{t("setup.looking")}</p>}
        </form>
      )}

      {step === "confirm" && found && (
        <div className="confirm-host" role="alertdialog" aria-labelledby="confirm-host-title">
          <h3 id="confirm-host-title">{t("setup.confirmTitle")}</h3>
          <p>{t("setup.confirmText", { host: found.imapHost })}</p>
          <div className="button-row">
            <button type="button" className="primary" onClick={() => void toCredentials(found)}>{t("setup.confirmYes")}</button>
            <button type="button" onClick={back}>{t("setup.confirmNo")}</button>
          </div>
        </div>
      )}

      {(step === "credentials" || step === "adding" || step === "waiting") && found && (
        <>
          <p>
            <strong>{name.trim()}</strong> &lt;{email.trim()}&gt;
          </p>
          <CredentialsStep
            settings={found}
            route={route}
            isBusy={step !== "credentials"}
            isWaitingForBrowser={step === "waiting"}
            onBack={back}
            onStop={() => void stopSignIn()}
            onPassword={(password) =>
              void run("adding", () => api.addAccount(name.trim(), email.trim(), password, found))
            }
            onOAuth={() =>
              void run("waiting", () =>
                api.addAccountOAuth(name.trim(), email.trim(), found, oauthDonePage(appInfo?.isPhone ?? false)),
              )
            }
          />
        </>
      )}
    </section>
  );
}
