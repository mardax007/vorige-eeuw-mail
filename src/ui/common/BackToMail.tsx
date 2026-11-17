import { t } from "../../i18n";
import { useApp } from "../state/AppContext";
import { useBackStep } from "./useBackStep";

/** The big button back to the mail screen, on every page that is not the mail itself. Android's
 *  back key does the same. */
export function BackToMail() {
  const { go } = useApp();
  useBackStep(true, () => go({ kind: "mail" }), 1);
  return (
    <button type="button" className="back-button" onClick={() => go({ kind: "mail" })}>
      ← {t("nav.backToMail")}
    </button>
  );
}
