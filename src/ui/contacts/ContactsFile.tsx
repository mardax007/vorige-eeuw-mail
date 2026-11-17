import { useState } from "react";
import { api } from "../../api/client";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

interface ContactsFileProps {
  /** Called after contacts came in from a file, so the list shows them. */
  onAdded: () => void;
}

/**
 * The address book as a .vcf file: saved where the user chooses, so it is never trapped in this
 * program, and read back from such a file (features.md, phase 2b task 3).
 */
export function ContactsFile({ onAdded }: ContactsFileProps) {
  const { showError } = useApp();
  const [isBusy, setIsBusy] = useState(false);
  const [result, setResult] = useState("");

  const run = async (work: () => Promise<string | null>) => {
    if (isBusy) return;
    setIsBusy(true);
    setResult("");
    try {
      const said = await work();
      if (said) setResult(said);
    } catch (e: unknown) {
      showError(e);
    } finally {
      setIsBusy(false);
    }
  };

  const save = () =>
    run(async () => {
      const path = await api.exportContacts(t("contacts.title"));
      return path ? t("contacts.file.saved", { path }) : null;
    });

  const add = () =>
    run(async () => {
      const r = await api.importContactsFile(false);
      if (!r) return null;
      onAdded();
      return r.found === 0 ? t("import.contacts.noneFound") : t("import.contacts.done", { added: r.added });
    });

  return (
    <section aria-labelledby="contacts-file">
      <h3 id="contacts-file">{t("contacts.file.title")}</h3>
      <p>{t("contacts.file.text")}</p>
      <div className="button-row">
        <button type="button" disabled={isBusy} onClick={() => void save()}>
          {t("contacts.file.save")}
        </button>
        <button type="button" disabled={isBusy} onClick={() => void add()}>
          {t("contacts.file.add")}
        </button>
      </div>
      {result && (
        <p className="calm-line" role="status">
          {result}
        </p>
      )}
    </section>
  );
}
