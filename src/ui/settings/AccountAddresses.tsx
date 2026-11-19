import { useState } from "react";
import { api } from "../../api/client";
import type { Account } from "../../api/types";
import { t } from "../../i18n";
import { isEmail } from "../compose/address";
import { useApp } from "../state/AppContext";

interface AccountAddressesProps {
  account: Account;
}

/**
 * "Andere adressen" of one mailbox (decision 32): addresses that also arrive in it, such as an
 * own domain at iCloud. Each one can be written from, and each other domain gets its own
 * Postvak IN in the folder pane.
 */
export function AccountAddresses({ account }: AccountAddressesProps) {
  const { reload, showError } = useApp();
  const [draft, setDraft] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const id = `addresses-${account.id}`;

  const save = async (next: string[]) => {
    setIsBusy(true);
    try {
      await api.setAccountAddresses(account.id, next);
      await reload();
      return true;
    } catch (e: unknown) {
      showError(e);
      return false;
    } finally {
      setIsBusy(false);
    }
  };

  const add = async () => {
    const address = draft.trim().toLowerCase();
    if (!address) return;
    if (!isEmail(address)) {
      setProblem(t("compose.badAddress", { address }));
      return;
    }
    if (address === account.email.toLowerCase() || account.addresses.includes(address)) {
      setProblem(t("settings.addressKnown", { address }));
      return;
    }
    setProblem(null);
    if (await save([...account.addresses, address])) setDraft("");
  };

  return (
    <section className="account-addresses" aria-labelledby={`${id}-title`}>
      <h5 id={`${id}-title`}>{t("settings.addresses")}</h5>
      <p className="hint">{t("settings.addressesHint")}</p>
      {account.addresses.length > 0 && (
        <ul className="address-list">
          {account.addresses.map((a) => (
            <li key={a}>
              <span>{a}</span>
              <button
                type="button"
                disabled={isBusy}
                aria-label={t("settings.addressRemoveNamed", { address: a })}
                onClick={() => void save(account.addresses.filter((x) => x !== a))}
              >
                {t("settings.addressRemove")}
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="form-row">
        <label htmlFor={id}>{t("settings.addressNew")}</label>
        <div className="input-with-button">
          <input
            id={id}
            type="email"
            autoComplete="off"
            enterKeyHint="done"
            value={draft}
            aria-invalid={problem ? true : undefined}
            aria-describedby={problem ? `${id}-problem` : undefined}
            onChange={(e) => {
              setDraft(e.target.value);
              setProblem(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") void add();
            }}
          />
          <button type="button" disabled={isBusy || draft.trim() === ""} onClick={() => void add()}>
            {t("settings.addressAdd")}
          </button>
        </div>
        {problem && (
          <p id={`${id}-problem`} className="form-problem" role="alert">
            {problem}
          </p>
        )}
      </div>
    </section>
  );
}
