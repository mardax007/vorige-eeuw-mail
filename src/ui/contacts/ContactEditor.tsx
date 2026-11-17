import { useRef, useState } from "react";
import type { Contact } from "../../api/types";
import { t } from "../../i18n";
import { isEmail } from "../compose/address";

interface ContactEditorProps {
  contact: Contact;
  onSave: (contact: Contact) => Promise<void> | void;
  onCancel: () => void;
}

/** Which field a problem belongs to: its text stands under that field, and focus goes there. */
interface Problem {
  field: "name" | number;
  text: string;
}

/** Name, one or more addresses, a phone number and a note. Nothing more. */
export function ContactEditor({ contact, onSave, onCancel }: ContactEditorProps) {
  const [name, setName] = useState(contact.name);
  const [emails, setEmails] = useState<string[]>(contact.emails.length > 0 ? contact.emails : [""]);
  const [phone, setPhone] = useState(contact.phone ?? "");
  const [note, setNote] = useState(contact.note ?? "");
  const [problem, setProblem] = useState<Problem | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);

  const showProblem = (next: Problem) => {
    setProblem(next);
    const id = next.field === "name" ? "contact-name" : `contact-email-${next.field}`;
    requestAnimationFrame(() => formRef.current?.querySelector<HTMLInputElement>(`#${id}`)?.focus());
  };

  const handleSave = async () => {
    if (isSaving) return;
    if (name.trim() === "") return showProblem({ field: "name", text: t("contacts.needName") });
    const badIndex = emails.findIndex((e) => e.trim() !== "" && !isEmail(e.trim()));
    if (badIndex >= 0) {
      return showProblem({ field: badIndex, text: t("compose.badAddress", { address: emails[badIndex].trim() }) });
    }
    setIsSaving(true);
    try {
      await onSave({
        ...contact,
        name: name.trim(),
        emails: emails.map((e) => e.trim()).filter((e) => e !== ""),
        phone: phone.trim() || null,
        note: note.trim() || null,
      });
    } finally {
      setIsSaving(false);
    }
  };

  const problemFor = (field: Problem["field"]) =>
    problem?.field === field ? (
      <p className="form-problem" id={`contact-problem-${field}`} role="alert">
        {problem.text}
      </p>
    ) : null;

  return (
    <form
      ref={formRef}
      className="contact-editor"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        void handleSave();
      }}
    >
      <div className="form-row">
        <label htmlFor="contact-name">{t("contacts.name")}</label>
        <input
          id="contact-name"
          value={name}
          autoComplete="off"
          autoCapitalize="words"
          aria-invalid={problem?.field === "name" || undefined}
          aria-describedby={problem?.field === "name" ? "contact-problem-name" : undefined}
          onChange={(e) => {
            setName(e.target.value);
            if (problem?.field === "name") setProblem(null);
          }}
        />
        {problemFor("name")}
      </div>
      <fieldset>
        <legend>{t("contacts.addresses")}</legend>
        {emails.map((email, i) => (
          <div className="form-row" key={i}>
            <label htmlFor={`contact-email-${i}`}>{t("contacts.address", { n: i + 1 })}</label>
            <input
              id={`contact-email-${i}`}
              type="email"
              inputMode="email"
              autoComplete="off"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              value={email}
              aria-invalid={problem?.field === i || undefined}
              aria-describedby={problem?.field === i ? `contact-problem-${i}` : undefined}
              onChange={(e) => {
                setEmails(emails.map((x, j) => (j === i ? e.target.value : x)));
                if (problem?.field === i) setProblem(null);
              }}
            />
            {problemFor(i)}
          </div>
        ))}
        <button type="button" onClick={() => setEmails([...emails, ""])}>{t("contacts.addAddress")}</button>
      </fieldset>
      <div className="form-row">
        <label htmlFor="contact-phone">{t("contacts.phone")}</label>
        <input id="contact-phone" type="tel" autoComplete="off" value={phone} onChange={(e) => setPhone(e.target.value)} />
      </div>
      <div className="form-row">
        <label htmlFor="contact-note">{t("contacts.note")}</label>
        <textarea id="contact-note" rows={3} value={note} onChange={(e) => setNote(e.target.value)} />
      </div>
      <div className="button-row">
        <button type="submit" className="primary" disabled={isSaving}>
          {isSaving ? t("general.saving") : t("contacts.save")}
        </button>
        <button type="button" disabled={isSaving} onClick={onCancel}>{t("contacts.cancel")}</button>
      </div>
    </form>
  );
}
