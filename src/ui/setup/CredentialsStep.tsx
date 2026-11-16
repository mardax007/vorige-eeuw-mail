import { useState } from "react";
import type { ServerSettings } from "../../api/types";
import { t } from "../../i18n";
import { PasswordField } from "../common/PasswordField";
import { SignInWait } from "../common/SignInWait";

export type SignInRoute = "password" | "oauth";

interface CredentialsStepProps {
  settings: ServerSettings;
  route: SignInRoute;
  isBusy: boolean;
  isWaitingForBrowser: boolean;
  onPassword: (password: string) => void;
  onOAuth: () => void;
  /** "Stoppen" while the browser sign-in is open. */
  onStop: () => void;
  onBack: () => void;
}

/** Whether the app can open this kind of mailbox with a password (decision 9: Microsoft cannot). */
export function canUsePassword(settings: ServerSettings): boolean {
  return settings.signIn !== "microsoft";
}

/** Second step of "Postvak toevoegen": the Google or Microsoft button, or the password. */
export function CredentialsStep({
  settings,
  route,
  isBusy,
  isWaitingForBrowser,
  onPassword,
  onOAuth,
  onStop,
  onBack,
}: CredentialsStepProps) {
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  if (route === "oauth") {
    const label = settings.signIn === "microsoft" ? t("setup.signInMicrosoft") : t("setup.signInGoogle");
    if (isWaitingForBrowser) return <SignInWait onStop={onStop} />;
    return (
      <div className="button-row">
        <button type="button" className="primary" disabled={isBusy} onClick={onOAuth}>
          {label}
        </button>
        <button type="button" disabled={isBusy} onClick={onBack}>{t("setup.back")}</button>
      </div>
    );
  }

  // A mailbox the program cannot open yet: say so, and only offer another address.
  if (!canUsePassword(settings)) {
    return (
      <div>
        <p className="calm-line">{t("setup.microsoftNotYet")}</p>
        <div className="button-row">
          <button type="button" className="primary" onClick={onBack}>{t("setup.back")}</button>
        </div>
      </div>
    );
  }

  const handleSubmit = () => {
    if (password === "") {
      setProblem(t("setup.missingPassword"));
      return;
    }
    setProblem(null);
    onPassword(password);
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (!isBusy) handleSubmit();
      }}
    >
      {settings.signIn === "google" && <p className="calm-line">{t("setup.googleAppPassword")}</p>}
      {problem && <p className="form-problem" role="alert">{problem}</p>}
      <PasswordField
        id="setup-password"
        label={t("setup.password")}
        value={password}
        autoComplete="current-password"
        onChange={setPassword}
      />
      <div className="button-row">
        <button type="submit" className="primary" disabled={isBusy}>
          {t("setup.add")}
        </button>
        <button type="button" disabled={isBusy} onClick={onBack}>{t("setup.back")}</button>
      </div>
      {isBusy && <p role="status">{t("setup.adding")}</p>}
    </form>
  );
}
