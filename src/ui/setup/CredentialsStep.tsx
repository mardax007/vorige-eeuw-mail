import { useState } from "react";
import type { ServerSettings } from "../../api/types";
import { t } from "../../i18n";

export type SignInRoute = "password" | "oauth";

interface CredentialsStepProps {
  settings: ServerSettings;
  route: SignInRoute;
  isBusy: boolean;
  isWaitingForBrowser: boolean;
  onPassword: (password: string) => void;
  onOAuth: () => void;
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
  onBack,
}: CredentialsStepProps) {
  const [password, setPassword] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  if (route === "oauth") {
    const label = settings.signIn === "microsoft" ? t("setup.signInMicrosoft") : t("setup.signInGoogle");
    return (
      <div>
        <div className="button-row">
          <button type="button" className="primary" disabled={isBusy} onClick={onOAuth}>
            {label}
          </button>
          <button type="button" disabled={isBusy} onClick={onBack}>{t("setup.back")}</button>
        </div>
        {isWaitingForBrowser && <p role="status">{t("setup.oauthWaiting")}</p>}
      </div>
    );
  }

  const isBlocked = !canUsePassword(settings);
  const note =
    settings.signIn === "google"
      ? t("setup.googleAppPassword")
      : settings.signIn === "microsoft"
        ? t("setup.microsoftNotYet")
        : null;

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
        if (!isBlocked) handleSubmit();
      }}
    >
      {note && <p className="calm-line">{note}</p>}
      {problem && <p className="form-problem" role="alert">{problem}</p>}
      <div className="form-row">
        <label htmlFor="setup-password">{t("setup.password")}</label>
        <input
          id="setup-password"
          type="password"
          autoComplete="current-password"
          value={password}
          disabled={isBlocked}
          onChange={(e) => setPassword(e.target.value)}
        />
      </div>
      <div className="button-row">
        <button type="submit" className="primary" disabled={isBusy || isBlocked}>
          {t("setup.add")}
        </button>
        <button type="button" disabled={isBusy} onClick={onBack}>{t("setup.back")}</button>
      </div>
      {isBusy && <p role="status">{t("setup.adding")}</p>}
    </form>
  );
}
