import { useState } from "react";
import { api } from "../../api/client";
import type { ContactsImported } from "../../api/types";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

type Phase = "idle" | "busy";

/** Contacts from Windows Live Mail: how to export them as business cards, then one button. */
export function ContactsFromLiveMail() {
  const { showError, go } = useApp();
  const [phase, setPhase] = useState<Phase>("idle");
  const [result, setResult] = useState<ContactsImported | null>(null);

  const pick = async (folder: boolean) => {
    if (phase === "busy") return;
    setPhase("busy");
    try {
      const r = await api.importContactsFile(folder);
      if (r) setResult(r);
    } catch (e: unknown) {
      showError(e);
    } finally {
      setPhase("idle");
    }
  };

  const already = result ? result.found - result.added : 0;
  const summary = result
    ? result.found === 0
      ? t("import.contacts.noneFound")
      : t("import.contacts.done", { added: result.added })
    : "";
  return (
    <section aria-labelledby="contacts-live-mail">
      <h3 id="contacts-live-mail">{t("import.contacts.title")}</h3>
      <p>{t("import.contacts.intro")}</p>
      <ol>
        <li>{t("import.contacts.step1")}</li>
        <li>{t("import.contacts.step2")}</li>
        <li>{t("import.contacts.step3")}</li>
        <li>{t("import.contacts.step4")}</li>
      </ol>
      <div className="button-row">
        <button type="button" disabled={phase === "busy"} onClick={() => void pick(true)}>
          {t("import.contacts.pickFolder")}
        </button>
        <button type="button" disabled={phase === "busy"} onClick={() => void pick(false)}>
          {t("import.contacts.pickFile")}
        </button>
      </div>
      {phase === "busy" && <p role="status">{t("import.contacts.busy")}</p>}
      {result && phase === "idle" && (
        <div role="status">
          <p>
            {summary}
            {already > 0 && ` ${t("import.contacts.alreadyThere", { count: already })}`}
          </p>
          {result.added > 0 && (
            <button type="button" onClick={() => go({ kind: "contacts" })}>
              {t("import.contacts.open")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
