import { useState } from "react";
import { t } from "../../i18n";

interface PasswordFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  autoComplete: "current-password" | "new-password";
  isDisabled?: boolean;
}

/** A password box with "Wachtwoord laten zien", so a typing mistake can be seen. */
export function PasswordField({ id, label, value, onChange, hint, autoComplete, isDisabled = false }: PasswordFieldProps) {
  const [isShown, setIsShown] = useState(false);
  const hintId = `${id}-hint`;
  return (
    <div className="form-row">
      <label htmlFor={id}>{label}</label>
      {hint && (
        <span id={hintId} className="hint">
          {hint}
        </span>
      )}
      <input
        id={id}
        type={isShown ? "text" : "password"}
        autoComplete={autoComplete}
        spellCheck={false}
        value={value}
        disabled={isDisabled}
        aria-describedby={hint ? hintId : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      <label className="check-row" htmlFor={`${id}-show`}>
        <input id={`${id}-show`} type="checkbox" checked={isShown} onChange={(e) => setIsShown(e.target.checked)} />
        {t("setup.showPassword")}
      </label>
    </div>
  );
}
