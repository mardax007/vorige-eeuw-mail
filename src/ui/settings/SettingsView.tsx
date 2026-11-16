import { t } from "../../i18n";
import { mailAccounts } from "../folders";
import { MAX_TEXT_SIZE, MIN_TEXT_SIZE, useApp } from "../state/AppContext";
import { AccountSettings } from "./AccountSettings";

export const TEXT_STEP = 2;
export const UNDO_SEND_CHOICES = [0, 5, 10, 20, 30] as const;

interface ToggleProps {
  id: string;
  label: string;
  isOn: boolean;
  onChange: (on: boolean) => void;
}

function Toggle({ id, label, isOn, onChange }: ToggleProps) {
  return (
    <label className="check-row" htmlFor={id}>
      <input id={id} type="checkbox" checked={isOn} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

/** "Instellingen": the few settings there are, on one page. */
export function SettingsView() {
  const { settings, updateSettings, accounts, go } = useApp();
  const size = settings.textSize;
  const setSize = (next: number) =>
    void updateSettings({ textSize: Math.min(MAX_TEXT_SIZE, Math.max(MIN_TEXT_SIZE, next)) });

  return (
    <section className="page settings">
      <h2>{t("settings.title")}</h2>

      <h3>{t("settings.textSize")}</h3>
      <div className="button-row">
        <button type="button" onClick={() => setSize(size - TEXT_STEP)} disabled={size <= MIN_TEXT_SIZE}>
          {t("settings.smaller")}
        </button>
        <button type="button" onClick={() => setSize(size + TEXT_STEP)} disabled={size >= MAX_TEXT_SIZE}>
          {t("settings.larger")}
        </button>
        <span>{t("settings.sizeNow", { size })}</span>
      </div>
      <p className="text-preview" style={{ fontSize: `${size}px` }}>
        {t("settings.preview")}
      </p>

      <Toggle
        id="set-notifications"
        label={t("settings.notifications")}
        isOn={settings.notifications}
        onChange={(on) => void updateSettings({ notifications: on })}
      />
      <Toggle
        id="set-start"
        label={t("settings.startWithWindows")}
        isOn={settings.startWithWindows}
        onChange={(on) => void updateSettings({ startWithWindows: on })}
      />
      <Toggle
        id="set-shrink"
        label={t("settings.shrinkPhotos")}
        isOn={settings.shrinkPhotos}
        onChange={(on) => void updateSettings({ shrinkPhotos: on })}
      />
      <div className="form-row">
        <label htmlFor="set-undo">{t("settings.undoSend")}</label>
        <select
          id="set-undo"
          value={settings.undoSendSeconds}
          onChange={(e) => void updateSettings({ undoSendSeconds: Number(e.target.value) })}
        >
          {UNDO_SEND_CHOICES.map((s) => (
            <option key={s} value={s}>
              {s === 0 ? t("settings.undoSendOff") : t("settings.undoSendSeconds", { seconds: s })}
            </option>
          ))}
        </select>
      </div>

      <h3>{t("settings.accounts")}</h3>
      {mailAccounts(accounts).map((a) => (
        <AccountSettings key={a.id} account={a} />
      ))}
      <div className="button-row">
        <button type="button" className="primary" onClick={() => go({ kind: "setup" })}>
          {t("settings.addAccount")}
        </button>
      </div>
    </section>
  );
}
