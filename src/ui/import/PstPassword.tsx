import { useId } from "react";
import { t } from "../../i18n";

type Props = {
  value: string;
  onChange: (value: string) => void;
  /** The wrong-password message, shown under the field. */
  problem: string | null;
};

/** The password of a protected Outlook file, asked before its import starts. */
export function PstPassword({ value, onChange, problem }: Props) {
  const id = useId();
  const problemId = `${id}-problem`;
  return (
    <div className="form-row">
      <p className="calm-line">{t("import.password.hint")}</p>
      <label htmlFor={id}>{t("import.password.label")}</label>
      <input
        id={id}
        type="password"
        autoComplete="off"
        value={value}
        aria-invalid={problem ? true : undefined}
        aria-describedby={problem ? problemId : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      {problem && (
        <p id={problemId} className="form-problem" role="alert">
          {problem}
        </p>
      )}
    </div>
  );
}
