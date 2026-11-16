import { useEffect, useRef } from "react";
import { t } from "./i18n";
import { BackToMail } from "./ui/common/BackToMail";
import { CheckMailButton } from "./ui/common/CheckMailButton";
import { ErrorBox } from "./ui/common/ErrorBox";
import { ReportNotice, SendBar, UndoBar, UpdateLine } from "./ui/common/Notices";
import { ComposeView } from "./ui/compose/ComposeView";
import { ContactsView } from "./ui/contacts/ContactsView";
import { HelpView } from "./ui/help/HelpView";
import { ImportView } from "./ui/import/ImportView";
import { MailView } from "./ui/mail/MailView";
import { PhoneView } from "./ui/phone/PhoneView";
import { SettingsView } from "./ui/settings/SettingsView";
import { AccountSetup } from "./ui/setup/AccountSetup";
import { AppProvider } from "./ui/state/AppProvider";
import { useApp, type View } from "./ui/state/AppContext";
import "./ui/styles/base.css";
import "./ui/styles/layout.css";
import "./ui/styles/mail.css";
import "./ui/styles/forms.css";

function TopBar() {
  const { go, compose, newDraft, view, accounts } = useApp();
  if (accounts.length === 0) return null;
  const isAt = (kind: View["kind"]) => view.kind === kind;
  // On the Write screen Versturen is the one main button; Nieuwe e-mail steps back.
  const newMailClass = isAt("compose") ? undefined : "primary";

  return (
    <header className="top-bar no-print">
      <h1 className="app-name">{t("app.name")}</h1>
      <div className="button-row">
        <button type="button" className={newMailClass} onClick={() => compose(newDraft())}>
          {t("nav.newMail")}
        </button>
        <CheckMailButton />
      </div>
      <div className="button-row top-right">
        <button type="button" aria-current={isAt("contacts") ? "page" : undefined} onClick={() => go({ kind: "contacts" })}>
          {t("nav.contacts")}
        </button>
        <button type="button" aria-current={isAt("settings") ? "page" : undefined} onClick={() => go({ kind: "settings" })}>
          {t("nav.settings")}
        </button>
        <button type="button" aria-current={isAt("help") ? "page" : undefined} onClick={() => go({ kind: "help" })}>
          {t("nav.help")}
        </button>
      </div>
    </header>
  );
}

function CurrentView() {
  const { view, go, accounts } = useApp();
  switch (view.kind) {
    case "mail":
      return <MailView />;
    case "compose":
      return <ComposeView key={view.key} initial={view.draft} onClose={() => go({ kind: "mail" })} />;
    case "setup":
      return <AccountSetup isFirstRun={accounts.length === 0} />;
    default:
      return (
        <div className="page-wrap">
          {/* Contactpersonen has its own back button, so an open contact is never left by accident. */}
          {view.kind !== "contacts" && <BackToMail />}
          {view.kind === "contacts" && <ContactsView />}
          {view.kind === "settings" && <SettingsView />}
          {view.kind === "help" && <HelpView />}
          {view.kind === "import" && <ImportView />}
          {view.kind === "phone" && <PhoneView />}
        </div>
      );
  }
}

/** Ctrl+N opens Nieuwe e-mail, as in Outlook. An extra; the button is always there. */
function useNewMailKey() {
  const { compose, newDraft, accounts } = useApp();
  const ref = useRef({ compose, newDraft, hasAccounts: accounts.length > 0 });
  useEffect(() => {
    ref.current = { compose, newDraft, hasAccounts: accounts.length > 0 };
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && (e.key === "n" || e.key === "N") && ref.current.hasAccounts) {
        e.preventDefault();
        ref.current.compose(ref.current.newDraft());
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

function Shell() {
  const { ready, error, dismissError } = useApp();
  useNewMailKey();
  if (!ready) {
    return (
      <main className="app loading" aria-busy="true">
        <p className="opening" role="status">{t("app.opening")}</p>
      </main>
    );
  }
  return (
    <div className="app">
      <TopBar />
      <UpdateLine />
      <ReportNotice />
      {error && (
        <div className="global-error">
          <ErrorBox error={error} onClose={dismissError} />
        </div>
      )}
      <main className="app-main">
        <CurrentView />
      </main>
      <div className="bottom-bars no-print">
        <UndoBar />
        <SendBar />
      </div>
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <Shell />
    </AppProvider>
  );
}
