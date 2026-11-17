import { useId } from "react";
import { t } from "../../i18n";
import { PasswordField } from "../common/PasswordField";

type Props = {
  value: string;
  onChange: (value: string) => void;
  /** The wrong-password message, shown under the field. */
  problem: string | null;
};

/** The password of a protected Outlook file, asked before its import starts. */
export function PstPassword({ value, onChange, problem }: Props) {
  const id = useId();
  return (
    <>
      <p className="calm-line">{t("import.password.hint")}</p>
      <PasswordField
        id={`pst-password-${id.replace(/:/g, "")}`}
        label={t("import.password.label")}
        value={value}
        autoComplete="off"
        problem={problem}
        onChange={onChange}
      />
    </>
  );
}
