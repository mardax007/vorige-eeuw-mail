import { useState } from "react";
import { t } from "../../i18n";

interface PasswordFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  /** "off" for a password that is not the mailbox's, such as a protected Outlook file's. */
  autoComplete: "current-password" | "new-password" | "off";
  isDisabled?: boolean;
  /** What is wrong with this field, shown right under it. */
  problem?: string | null;
}

/** A password box with "Wachtwoord laten zien", so a typing mistake can be seen. */
export function PasswordField({
  id,
  label,
  value,
  onChange,
  hint,
  autoComplete,
  isDisabled = false,
  problem = null,
}: PasswordFieldProps) {
  const [isShown, setIsShown] = useState(false);
  const hintId = `${id}-hint`;
  const problemId = `${id}-problem`;
  const describedBy = [hint ? hintId : null, problem ? problemId : null].filter(Boolean).join(" ");
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
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        value={value}
        disabled={isDisabled}
        aria-invalid={problem ? true : undefined}
        aria-describedby={describedBy || undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      {problem && (
        <p id={problemId} className="form-problem" role="alert">
          {problem}
        </p>
      )}
      <label className="check-row" htmlFor={`${id}-show`}>
        <input id={`${id}-show`} type="checkbox" checked={isShown} onChange={(e) => setIsShown(e.target.checked)} />
        {t("setup.showPassword")}
      </label>
    </div>
  );
}
