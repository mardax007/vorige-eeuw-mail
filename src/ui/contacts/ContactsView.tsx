import { useCallback, useEffect, useState } from "react";
import { api } from "../../api/client";
import type { Contact, MessageSummary } from "../../api/types";
import { t } from "../../i18n";
import { BackToMail } from "../common/BackToMail";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { useBackStep } from "../common/useBackStep";
import { shortDate } from "../format";
import { useApp } from "../state/AppContext";
import { ContactEditor } from "./ContactEditor";

const EMPTY_CONTACT: Contact = { id: "", name: "", emails: [], phone: null, note: null };

type Mode = { kind: "list" } | { kind: "show"; contact: Contact } | { kind: "edit"; contact: Contact };

function byName(list: readonly Contact[]): Contact[] {
  return [...list].sort((a, b) => a.name.localeCompare(b.name, "nl"));
}

/** Contactpersonen: one list sorted by name, a search box, and one person at a time. */
export function ContactsView() {
  const { showError, compose, newDraft, setLocation, setOpenMessageId, go } = useApp();
  const [query, setQuery] = useState("");
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [hasLoadError, setHasLoadError] = useState(false);
  const [mode, setMode] = useState<Mode>({ kind: "list" });
  // null while it loads; "failed" when it could not (the error stands at the top).
  const [recent, setRecent] = useState<MessageSummary[] | null | "failed">(null);
  const [isConfirming, setIsConfirming] = useState(false);

  const load = useCallback(async () => {
    setIsLoading(true);
    try {
      setContacts(byName(await api.listContacts(query)));
      setHasLoadError(false);
    } catch (e: unknown) {
      setHasLoadError(true);
      showError(e);
    } finally {
      setIsLoading(false);
    }
  }, [query, showError]);

  useEffect(() => {
    void load();
  }, [load]);

  const show = async (contact: Contact) => {
    setMode({ kind: "show", contact });
    setRecent(null);
    try {
      setRecent(await api.contactMessages(contact.id));
    } catch (e: unknown) {
      setRecent("failed");
      showError(e);
    }
  };

  const save = async (contact: Contact) => {
    try {
      const saved = await api.saveContact(contact);
      await load();
      await show(saved);
    } catch (e: unknown) {
      showError(e);
    }
  };

  const remove = async (contact: Contact) => {
    setIsConfirming(false);
    try {
      await api.deleteContact(contact.id);
      setMode({ kind: "list" });
      await load();
    } catch (e: unknown) {
      showError(e);
    }
  };

  const openMessage = (m: MessageSummary) => {
    setLocation({ kind: "folder", folderId: m.folderId });
    setOpenMessageId(m.id);
    go({ kind: "mail" });
  };

  // Annuleren while editing: back to the contact, or to the list for a new one.
  const stepBack = () => {
    if (mode.kind === "edit" && mode.contact.id) setMode({ kind: "show", contact: mode.contact });
    else setMode({ kind: "list" });
  };
  // Android's back key: on an open contact it is Terug. While editing it does nothing, so a slip
  // of the thumb never throws away what was typed (and never closes the app).
  useBackStep(
    mode.kind !== "list",
    () => {
      if (mode.kind === "show") setMode({ kind: "list" });
    },
    2,
  );

  if (mode.kind === "edit") {
    return (
      <section className="page">
        <h2>{mode.contact.id ? mode.contact.name : t("contacts.new")}</h2>
        <ContactEditor contact={mode.contact} onSave={save} onCancel={stepBack} />
      </section>
    );
  }

  if (mode.kind === "show") {
    const c = mode.contact;
    return (
      <section className="page">
        <button type="button" className="back-button" onClick={() => setMode({ kind: "list" })}>← {t("contacts.back")}</button>
        <h2>{c.name}</h2>
        <ul className="plain-list">
          {c.emails.map((e) => <li key={e}>{e}</li>)}
          {c.phone && <li>{t("contacts.phone")}: {c.phone}</li>}
          {c.note && <li>{c.note}</li>}
        </ul>
        <div className="button-row">
          {c.emails.length > 0 && (
            <button type="button" className="primary" onClick={() => compose(newDraft({ to: [{ name: c.name, email: c.emails[0] }] }))}>
              {t("contacts.write")}
            </button>
          )}
          <button type="button" onClick={() => setMode({ kind: "edit", contact: c })}>{t("contacts.edit")}</button>
          <button type="button" className="danger-outline" onClick={() => setIsConfirming(true)}>{t("contacts.delete")}</button>
        </div>
        <h3>{t("contacts.recent")}</h3>
        {recent === null && <p role="status">{t("contacts.recentLoading")}</p>}
        {recent === "failed" && <p>{t("contacts.recentFailed")}</p>}
        {Array.isArray(recent) && recent.length === 0 && <p>{t("contacts.noRecent")}</p>}
        <ul className="plain-list">
          {(Array.isArray(recent) ? recent : []).map((m) => (
            <li key={m.id}>
              <button type="button" className="list-button" onClick={() => openMessage(m)}>
                {shortDate(m.date)} · {m.subject || t("list.noSubject")}
              </button>
            </li>
          ))}
        </ul>
        {isConfirming && (
          <ConfirmDialog
            text={t("contacts.deleteConfirm", { name: c.name })}
            confirmLabel={t("contacts.deleteYes")}
            onConfirm={() => void remove(c)}
            onCancel={() => setIsConfirming(false)}
          />
        )}
      </section>
    );
  }

  return (
    <section className="page">
      <BackToMail />
      <h2>{t("contacts.title")}</h2>
      <div className="button-row">
        <button type="button" className="primary" onClick={() => setMode({ kind: "edit", contact: EMPTY_CONTACT })}>
          {t("contacts.new")}
        </button>
      </div>
      <div className="form-row">
        <label htmlFor="contact-search">{t("contacts.search")}</label>
        <input id="contact-search" type="search" value={query} onChange={(e) => setQuery(e.target.value)} />
      </div>
      {isLoading && contacts.length === 0 && <p role="status">{t("contacts.loading")}</p>}
      {!isLoading && hasLoadError && contacts.length === 0 && (
        <div className="button-row">
          <p>{t("contacts.loadFailed")}</p>
          <button type="button" onClick={() => void load()}>{t("list.retry")}</button>
        </div>
      )}
      {!isLoading && !hasLoadError && contacts.length === 0 && (
        <p>{query.trim() === "" ? t("contacts.empty") : t("contacts.none", { query: query.trim() })}</p>
      )}
      <ul className="plain-list">
        {contacts.map((c) => (
          <li key={c.id}>
            <button type="button" className="list-button" onClick={() => void show(c)}>
              <strong>{c.name}</strong> <span className="row-address">{c.emails[0] ?? ""}</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
