import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { isStoppedSignIn, toVemError } from "../../api/errors";
import type { Account, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { PasswordField } from "../common/PasswordField";
import { SignInWait } from "../common/SignInWait";
import { useApp } from "../state/AppContext";
import { providerOf, signInRoute } from "./signIn";

interface AccountSignInProps {
  account: Account;
  /** The mailbox's current error, if checking it failed. */
  currentError: VemError | null;
}

type Phase = "idle" | "busy" | "done";

/** Ask the core to fetch again after a new password or sign-in, so the old error goes away. */
function useRefresh(accountId: string) {
  const { reload, showError } = useApp();
  return async () => {
    try {
      await api.syncNow(accountId);
      await reload();
    } catch (e: unknown) {
      showError(e);
    }
  };
}

/** "Nieuw wachtwoord" for a password mailbox. Nothing is saved unless the new password works. */
function NewPassword({ account }: { account: Account }) {
  const refresh = useRefresh(account.id);
  const [password, setPassword] = useState("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [problem, setProblem] = useState<string | null>(null);
  const [error, setError] = useState<VemError | null>(null);
  const id = `password-${account.id}`;

  const save = async () => {
    setProblem(null);
    setError(null);
    if (password === "") {
      setProblem(t("settings.missingPassword"));
      setPhase("idle");
      return;
    }
    setPhase("busy");
    try {
      await api.updatePassword(account.id, password);
      setPassword("");
      setPhase("done");
      await refresh();
    } catch (e: unknown) {
      setError(toVemError(e));
      setPhase("idle");
    }
  };

  return (
    <form
      className="account-sign-in"
      onSubmit={(e) => {
        e.preventDefault();
        if (phase !== "busy") void save();
      }}
    >
      <PasswordField
        id={id}
        label={t("settings.newPassword", { account: account.displayName })}
        hint={t("settings.newPasswordHint")}
        value={password}
        autoComplete="new-password"
        isDisabled={phase === "busy"}
        onChange={(value) => {
          setPassword(value);
          if (phase === "done") setPhase("idle");
        }}
      />
      {problem && <p className="form-problem" role="alert">{problem}</p>}
      {error && <ErrorBox error={error} onClose={() => setError(null)} />}
      <div className="button-row">
        <button type="submit" disabled={phase === "busy"}>{t("settings.savePassword")}</button>
        <span role="status">
          {phase === "busy" ? t("settings.checkingPassword") : phase === "done" ? t("settings.passwordSaved") : ""}
        </span>
      </div>
    </form>
  );
}

/** "Opnieuw inloggen" for a Google or Microsoft mailbox, with "Stoppen" while the browser is open. */
function SignInAgain({ account }: { account: Account }) {
  const refresh = useRefresh(account.id);
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<VemError | null>(null);
  const isStopping = useRef(false);
  const isBusy = useRef(false);

  useEffect(() => {
    isBusy.current = phase === "busy";
  }, [phase]);

  // Leaving the page while the browser is open: stop that sign-in too, quietly.
  useEffect(
    () => () => {
      if (isBusy.current) api.cancelSignIn().catch(() => undefined);
    },
    [],
  );

  const start = async () => {
    setError(null);
    isStopping.current = false;
    setPhase("busy");
    try {
      await api.reauthAccount(account.id, t("setup.oauthDonePage"));
      setPhase("done");
      await refresh();
    } catch (e: unknown) {
      const err = toVemError(e);
      // Stopped by the user: just go back, no error.
      if (!isStopping.current && !isStoppedSignIn(err)) setError(err);
      setPhase("idle");
    }
  };

  const stop = async () => {
    isStopping.current = true;
    try {
      await api.cancelSignIn();
    } catch (e: unknown) {
      // Not stopped: a later failure of this sign-in is a real one again.
      isStopping.current = false;
      setError(toVemError(e));
    }
  };

  return (
    <div className="account-sign-in">
      <p>{t("settings.reauthText")}</p>
      {error && <ErrorBox error={error} onClose={() => setError(null)} />}
      {phase === "busy" ? (
        <SignInWait onStop={() => void stop()} />
      ) : (
        <div className="button-row">
          <button type="button" onClick={() => void start()}>{t("settings.reauth")}</button>
          {phase === "done" && <span role="status">{t("settings.reauthDone")}</span>}
        </div>
      )}
    </div>
  );
}

/** How this mailbox signs in again, with its current error right above. */
export function AccountSignIn({ account, currentError }: AccountSignInProps) {
  // The core says how the mailbox signed in; only an older index without it falls back to the address.
  const isKnown = account.signIn != null;
  const provider = account.signIn ?? providerOf(account.email);
  const [isOAuthAvailable, setIsOAuthAvailable] = useState(false);

  useEffect(() => {
    if (isKnown || provider === "password") return;
    let alive = true;
    api
      .oauthAvailable(provider)
      .then((available) => {
        if (alive) setIsOAuthAvailable(available);
      })
      // Unknown: offer the password, which an app password also fits.
      .catch(() => {
        if (alive) setIsOAuthAvailable(false);
      });
    return () => {
      alive = false;
    };
  }, [isKnown, provider]);

  // A mailbox that signed in through the browser signs in that way again.
  const route = signInRoute(provider, isKnown || isOAuthAvailable, currentError);
  return (
    <>
      {currentError && <ErrorBox error={currentError} />}
      {route === "browser" ? <SignInAgain account={account} /> : <NewPassword account={account} />}
    </>
  );
}
