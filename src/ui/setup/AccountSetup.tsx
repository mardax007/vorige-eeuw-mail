import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { isStoppedSignIn, toVemError } from "../../api/errors";
import type { Account, ServerSettings, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { useBackStep } from "../common/useBackStep";
import { isEmail } from "../compose/address";
import { oauthDonePage } from "../settings/signIn";
import { useApp } from "../state/AppContext";
import { CredentialsStep, type SignInRoute } from "./CredentialsStep";

/** address → (confirm a guessed host) → credentials → adding or waiting for the browser. */
type Step = "address" | "looking" | "confirm" | "credentials" | "adding" | "waiting";

interface AccountSetupProps {
  isFirstRun: boolean;
}

/** A problem with the name or the address, shown under that field. */
interface Problem {
  field: "name" | "email";
  text: string;
}

/** "Postvak toevoegen": name and address, then a password or the Google/Microsoft sign-in. */
export function AccountSetup({ isFirstRun }: AccountSetupProps) {
  const { reload, go, setLocation, appInfo } = useApp();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [step, setStep] = useState<Step>("address");
  const [found, setFound] = useState<ServerSettings | null>(null);
  const [route, setRoute] = useState<SignInRoute>("password");
  const [problem, setProblem] = useState<Problem | null>(null);
  const [error, setError] = useState<VemError | null>(null);
  const isStopping = useRef(false);
  const errorRef = useRef<HTMLDivElement>(null);
  const confirmTitleRef = useRef<HTMLHeadingElement>(null);

  // A failure is said at the top; on a small screen the top may be out of view after pressing.
  useEffect(() => {
    if (error) errorRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [error]);

  useEffect(() => {
    if (step === "confirm") confirmTitleRef.current?.focus();
  }, [step]);

  // Android's back key: from the first step back to Instellingen (as Annuleren), from a later
  // step back to the first. Not on the very first start, where there is nothing to go back to.
  useBackStep(!isFirstRun || step !== "address", () => {
    if (step === "looking" || step === "adding" || step === "waiting") return;
    if (step === "address") go({ kind: "settings" });
    else back();
  }, 1);

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

  const showProblem = (next: Problem) => {
    setProblem(next);
    requestAnimationFrame(() => document.getElementById(next.field === "name" ? "setup-name" : "setup-email")?.focus());
  };

  const handleLookup = async () => {
    setProblem(null);
    setError(null);
    if (name.trim() === "") return showProblem({ field: "name", text: t("setup.missingName") });
    if (email.trim() === "") return showProblem({ field: "email", text: t("setup.missingEmail") });
    if (!isEmail(email.trim())) return showProblem({ field: "email", text: t("setup.badEmail") });
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
      <div ref={errorRef}>{error && <ErrorBox error={error} onClose={() => setError(null)} />}</div>

      {(step === "address" || step === "looking") && (
        <form
          noValidate
          onSubmit={(e) => {
            e.preventDefault();
            void handleLookup();
          }}
        >
          <div className="form-row">
            <label htmlFor="setup-name">{t("setup.name")}</label>
            <input
              id="setup-name"
              autoComplete="name"
              autoCapitalize="words"
              enterKeyHint="next"
              value={name}
              aria-invalid={problem?.field === "name" || undefined}
              aria-describedby={problem?.field === "name" ? "setup-name-problem setup-name-hint" : "setup-name-hint"}
              onChange={(e) => {
                setName(e.target.value);
                if (problem?.field === "name") setProblem(null);
              }}
            />
            <span className="hint" id="setup-name-hint">{t("setup.nameHint")}</span>
            {problem?.field === "name" && (
              <p className="form-problem" id="setup-name-problem" role="alert">{problem.text}</p>
            )}
          </div>
          <div className="form-row">
            <label htmlFor="setup-email">{t("setup.email")}</label>
            <input
              id="setup-email"
              type="email"
              inputMode="email"
              autoComplete="email"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              enterKeyHint="go"
              value={email}
              aria-invalid={problem?.field === "email" || undefined}
              aria-describedby={problem?.field === "email" ? "setup-email-problem" : undefined}
              onChange={(e) => {
                setEmail(e.target.value);
                if (problem?.field === "email") setProblem(null);
              }}
            />
            {problem?.field === "email" && (
              <p className="form-problem" id="setup-email-problem" role="alert">{problem.text}</p>
            )}
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
        <div className="confirm-host" role="group" aria-labelledby="confirm-host-title">
          <h3 id="confirm-host-title" ref={confirmTitleRef} tabIndex={-1}>{t("setup.confirmTitle")}</h3>
          <p>{t("setup.confirmText", { host: found.imapHost })}</p>
          {/* The password would go to a server nobody looked up: the safe answer comes first. */}
          <div className="button-row">
            <button type="button" onClick={back}>{t("setup.confirmNo")}</button>
            <button type="button" onClick={() => void toCredentials(found)}>{t("setup.confirmYes")}</button>
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
