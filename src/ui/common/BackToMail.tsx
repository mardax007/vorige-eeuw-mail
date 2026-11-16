import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

/** The big button back to the mail screen, on every page that is not the mail itself. */
export function BackToMail() {
  const { go } = useApp();
  return (
    <button type="button" className="back-button" onClick={() => go({ kind: "mail" })}>
      ← {t("nav.backToMail")}
    </button>
  );
}
