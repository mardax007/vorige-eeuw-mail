import { t } from "../../i18n";

interface SignInWaitProps {
  onStop: () => void;
}

/** While the browser sign-in is open: what to do there, and "Stoppen" to give up and go back. */
export function SignInWait({ onStop }: SignInWaitProps) {
  return (
    <div className="sign-in-wait">
      <p role="status">{t("setup.oauthWaiting")}</p>
      <button type="button" onClick={onStop}>
        {t("setup.stop")}
      </button>
    </div>
  );
}
