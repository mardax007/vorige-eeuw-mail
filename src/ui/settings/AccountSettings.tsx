import { useState } from "react";
import { api } from "../../api/client";
import type { Account, Quota } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { formatKb } from "../format";
import { useApp } from "../state/AppContext";
import { AccountAddresses } from "./AccountAddresses";
import { AccountSignIn } from "./AccountSignIn";

/** From this share of the mailbox a calm warning shows (features.md: 90%). */
export const QUOTA_WARN = 0.9;

interface QuotaBarProps {
  quota: Quota;
}

/** "Ruimte in postvak": used of limit, as a bar and in words. */
export function QuotaBar({ quota }: QuotaBarProps) {
  const used = formatKb(quota.usedKb);
  if (!quota.limitKb) return <p>{t("settings.spaceUnknown", { used })}</p>;
  const share = Math.min(1, quota.usedKb / quota.limitKb);
  return (
    <div className="quota">
      <p>{t("settings.spaceUsed", { used, limit: formatKb(quota.limitKb) })}</p>
      <progress max={100} value={Math.round(share * 100)} aria-label={t("settings.space")} />
      {share >= QUOTA_WARN && <p className="mail-warning">{t("settings.spaceFull")}</p>}
    </div>
  );
}

interface AccountSettingsProps {
  account: Account;
}

/** One mailbox on the settings page: its space, its signature, signing in again, and removing it. */
export function AccountSettings({ account }: AccountSettingsProps) {
  const { reload, showError, accounts, go, syncErrors } = useApp();
  const [signature, setSignature] = useState(account.signature ?? "");
  const [isSaved, setIsSaved] = useState(false);
  const [isConfirming, setIsConfirming] = useState(false);
  const id = `signature-${account.id}`;

  const saveSignature = async () => {
    try {
      await api.setSignature(account.id, signature);
      setIsSaved(true);
      await reload();
    } catch (e: unknown) {
      showError(e);
    }
  };

  const remove = async () => {
    setIsConfirming(false);
    try {
      await api.removeAccount(account.id);
      await reload();
      if (accounts.length <= 1) go({ kind: "setup" });
    } catch (e: unknown) {
      showError(e);
    }
  };

  return (
    <div className="account-settings">
      <h4>
        {account.displayName} <span className="row-address">&lt;{account.email}&gt;</span>
      </h4>
      {account.quota && (
        <>
          <h5>{t("settings.space")}</h5>
          <QuotaBar quota={account.quota} />
        </>
      )}
      <div className="form-row">
        <label htmlFor={id}>{t("settings.signatureFor", { account: account.displayName })}</label>
        <textarea
          id={id}
          rows={3}
          value={signature}
          onChange={(e) => {
            setSignature(e.target.value);
            setIsSaved(false);
          }}
          // Leaving the field keeps what was typed, also without pressing the button.
          onBlur={() => {
            if (signature !== (account.signature ?? "")) void saveSignature();
          }}
        />
      </div>
      <div className="button-row">
        <button type="button" onClick={saveSignature}>{t("settings.saveSignature")}</button>
        {isSaved && <span role="status">{t("settings.signatureSaved")}</span>}
      </div>
      {account.kind === "imap" && <AccountAddresses account={account} />}
      <AccountSignIn account={account} currentError={syncErrors[account.id] ?? account.lastError ?? null} />
      <div className="button-row danger-zone">
        <button type="button" className="danger-outline" onClick={() => setIsConfirming(true)}>
          {t("settings.removeAccount")}
        </button>
      </div>
      {isConfirming && (
        <ConfirmDialog
          text={t("settings.removeConfirm", { account: account.displayName })}
          confirmLabel={t("settings.removeYes")}
          onConfirm={() => void remove()}
          onCancel={() => setIsConfirming(false)}
        />
      )}
    </div>
  );
}
