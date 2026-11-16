import { useState } from "react";
import type { Contact } from "../../api/types";
import { t } from "../../i18n";
import { isEmail } from "../compose/address";

interface ContactEditorProps {
  contact: Contact;
  onSave: (contact: Contact) => void;
  onCancel: () => void;
}

/** Name, one or more addresses, a phone number and a note. Nothing more. */
export function ContactEditor({ contact, onSave, onCancel }: ContactEditorProps) {
  const [name, setName] = useState(contact.name);
  const [emails, setEmails] = useState<string[]>(contact.emails.length > 0 ? contact.emails : [""]);
  const [phone, setPhone] = useState(contact.phone ?? "");
  const [note, setNote] = useState(contact.note ?? "");
  const [problem, setProblem] = useState<string | null>(null);

  const handleSave = () => {
    const cleaned = emails.map((e) => e.trim()).filter((e) => e !== "");
    const bad = cleaned.find((e) => !isEmail(e));
    if (name.trim() === "") return setProblem(t("contacts.needName"));
    if (bad) return setProblem(t("compose.badAddress", { address: bad }));
    onSave({
      ...contact,
      name: name.trim(),
      emails: cleaned,
      phone: phone.trim() || null,
      note: note.trim() || null,
    });
  };

  return (
    <form
      className="contact-editor"
      onSubmit={(e) => {
        e.preventDefault();
        handleSave();
      }}
    >
      {problem && <p className="form-problem" role="alert">{problem}</p>}
      <div className="form-row">
        <label htmlFor="contact-name">{t("contacts.name")}</label>
        <input id="contact-name" value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <fieldset>
        <legend>{t("contacts.addresses")}</legend>
        {emails.map((email, i) => (
          <div className="form-row" key={i}>
            <label htmlFor={`contact-email-${i}`}>{t("contacts.address", { n: i + 1 })}</label>
            <input
              id={`contact-email-${i}`}
              type="email"
              value={email}
              onChange={(e) => setEmails(emails.map((x, j) => (j === i ? e.target.value : x)))}
            />
          </div>
        ))}
        <button type="button" onClick={() => setEmails([...emails, ""])}>{t("contacts.addAddress")}</button>
      </fieldset>
      <div className="form-row">
        <label htmlFor="contact-phone">{t("contacts.phone")}</label>
        <input id="contact-phone" type="tel" value={phone} onChange={(e) => setPhone(e.target.value)} />
      </div>
      <div className="form-row">
        <label htmlFor="contact-note">{t("contacts.note")}</label>
        <textarea id="contact-note" rows={3} value={note} onChange={(e) => setNote(e.target.value)} />
      </div>
      <div className="button-row">
        <button type="submit" className="primary">{t("contacts.save")}</button>
        <button type="button" onClick={onCancel}>{t("contacts.cancel")}</button>
      </div>
    </form>
  );
}
