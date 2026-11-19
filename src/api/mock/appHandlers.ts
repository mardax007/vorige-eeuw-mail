import type { CommandHandlers } from "../commands";
import type { Account, ServerSettings, SignIn } from "../types";
import type { MockFolder, MockStore } from "./state";
import { mockError } from "./state";

type AppCommands =
  | "app_info"
  | "update_offer"
  | "install_update"
  | "list_accounts"
  | "lookup_settings"
  | "add_account"
  | "set_shell_texts"
  | "oauth_available"
  | "add_account_oauth"
  | "update_password"
  | "reauth_account"
  | "cancel_sign_in"
  | "remove_account"
  | "set_signature"
  | "set_account_addresses"
  | "sync_now"
  | "get_settings"
  | "save_settings"
  | "send_wish"
  | "help_report"
  | "pair_phone"
  | "phone_paired"
  | "phone_last_copy"
  | "pair_with_code"
  | "unpair_phone"
  | "quit_app";

/** A made-up QR-like picture; the real one comes from the core. */
const SAMPLE_QR =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 21 21" shape-rendering="crispEdges">' +
  '<rect width="21" height="21" fill="#fff"/>' +
  '<path fill="#000" d="M0 0h7v7H0zM14 0h7v7h-7zM0 14h7v7H0zM9 2h2v2H9zM9 9h3v3H9zM14 14h2v2h-2zM17 17h3v3h-3zM12 15h1v4h-1z"/>' +
  '<path fill="#fff" d="M1 1h5v5H1zM15 1h5v5h-5zM1 15h5v5H1z"/>' +
  '<path fill="#000" d="M2 2h3v3H2zM16 2h3v3h-3zM2 16h3v3H2z"/></svg>';

const MICROSOFT_DOMAINS = ["outlook.com", "hotmail.com", "live.nl", "outlook.nl"];

/** How long the mock pretends the user takes to sign in in the browser. */
const OAUTH_WAIT_MS = 1000;

function settingsFor(email: string): ServerSettings {
  const domain = email.split("@")[1]?.toLowerCase() ?? "";
  if (domain === "" || domain.startsWith("onbekend")) {
    throw mockError("VEM-LOOKUP-FIND-01", undefined, domain);
  }
  const microsoft = MICROSOFT_DOMAINS.includes(domain);
  const known = domain === "solcon.nl" || domain === "gmail.com" || microsoft;
  return {
    imapHost: known ? `imap.${domain}` : "mail.hosting-voorbeeld.example",
    imapPort: 993,
    imapTls: "tls",
    smtpHost: known ? `smtp.${domain}` : "mail.hosting-voorbeeld.example",
    smtpPort: 465,
    smtpTls: "tls",
    username: email,
    source: known ? "table" : "guess",
    needsConfirmation: !known,
    signIn: domain === "gmail.com" ? "google" : microsoft ? "microsoft" : "password",
  };
}

function standardFolders(accountId: string): MockFolder[] {
  const roles = [
    ["INBOX", "inbox"],
    ["Sent", "sent"],
    ["Trash", "trash"],
    ["Drafts", "drafts"],
  ] as const;
  return roles.map(([name, role]) => ({ id: `${accountId}-${role}`, accountId, name, role, path: [name] }));
}

function createAccount(store: MockStore, displayName: string, email: string, signIn: SignIn): Account {
  const account: Account = {
    id: store.nextId("acc"),
    displayName,
    email,
    kind: "imap",
    quota: { usedKb: 120_000, limitKb: 1_000_000 },
    signature: null,
    lastError: null,
    signIn,
    addresses: [],
  };
  store.set((s) => ({
    ...s,
    accounts: [...s.accounts, account],
    folders: [...s.folders, ...standardFolders(account.id)],
  }));
  return account;
}

/** A password the mock refuses, to show what a failed login looks like. */
const WRONG_PASSWORD = "fout";

function loginRefused() {
  return mockError("VEM-IMAP-LOGIN-01", "a1 NO [AUTHENTICATIONFAILED] Authentication failed.");
}

interface PendingSignIn {
  timer: ReturnType<typeof setTimeout>;
  reject: (reason: unknown) => void;
}

export function createAppHandlers(store: MockStore): Pick<CommandHandlers, AppCommands> {
  let pending: PendingSignIn | null = null;

  /** Pretends the user signs in in the browser; cancel_sign_in stops it. */
  const waitForBrowser = <T>(finish: () => T): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      const timer = store.later(OAUTH_WAIT_MS, () => {
        pending = null;
        resolve(finish());
      });
      pending = { timer, reject };
    });

  const clearAccountError = (accountId: string) => {
    store.set((s) => ({
      ...s,
      accounts: s.accounts.map((a) => (a.id === accountId ? { ...a, lastError: null } : a)),
    }));
    store.events.emit("sync:state", { accountId, busy: false, error: null });
  };

  return {
    // The browser preview is no phone: nothing to offer, and Bijwerken does nothing.
    update_offer: () => null,
    install_update: () => undefined,
    app_info: () => ({
      version: "0.1.0",
      updatedTo: "0.1.0",
      whatsNew: [
        "U kunt nu een bericht een label geven.",
        "Zoeken gaat sneller.",
      ],
    }),
    list_accounts: () => store.get().accounts,
    lookup_settings: ({ email }) => settingsFor(email),
    add_account: ({ displayName, email, password }) => {
      if (password === WRONG_PASSWORD) throw loginRefused();
      return createAccount(store, displayName, email, "password");
    },
    set_shell_texts: () => undefined,
    oauth_available: ({ signIn }) => signIn === "google",
    add_account_oauth: ({ displayName, email, settings }) =>
      waitForBrowser<Account>(() => createAccount(store, displayName, email, settings.signIn)),
    update_password: ({ accountId, password }) => {
      if (!store.get().accounts.some((a) => a.id === accountId)) {
        throw mockError("VEM-STORE-READ-01", undefined, `account ${accountId}`);
      }
      if (password === WRONG_PASSWORD) throw loginRefused();
      clearAccountError(accountId);
    },
    reauth_account: ({ accountId }) => waitForBrowser(() => clearAccountError(accountId)),
    cancel_sign_in: () => {
      if (!pending) return;
      store.cancel(pending.timer);
      pending.reject(mockError("VEM-AUTH-LOGIN-01", undefined, "stopped"));
      pending = null;
    },
    remove_account: ({ accountId }) =>
      store.set((s) => ({
        ...s,
        accounts: s.accounts.filter((a) => a.id !== accountId),
        folders: s.folders.filter((f) => f.accountId !== accountId),
        messages: s.messages.filter((m) => m.accountId !== accountId),
      })),
    set_account_addresses: ({ accountId, addresses }) => {
      const account = store.get().accounts.find((a) => a.id === accountId);
      if (!account) throw mockError("VEM-APP-CMD-01", undefined, "no such account");
      const list: string[] = [];
      for (const raw of addresses) {
        const a = raw.trim().toLowerCase();
        if (!a || a === account.email.toLowerCase() || list.includes(a)) continue;
        if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(a)) throw mockError("VEM-APP-CMD-01", undefined, `not an address: ${a}`);
        list.push(a);
      }
      store.set((s) => ({ ...s, accounts: s.accounts.map((x) => (x.id === accountId ? { ...x, addresses: list } : x)) }));
      return list;
    },
    set_signature: ({ accountId, signature }) =>
      store.set((s) => ({
        ...s,
        accounts: s.accounts.map((a) => (a.id === accountId ? { ...a, signature } : a)),
      })),
    sync_now: ({ accountId }) => {
      const ids = accountId ? [accountId] : store.get().accounts.map((a) => a.id);
      ids.forEach((id) => store.events.emit("sync:state", { accountId: id, busy: true, error: null }));
      store.later(300, () =>
        ids.forEach((id) => store.events.emit("sync:state", { accountId: id, busy: false, error: null })),
      );
    },
    get_settings: () => store.get().settings,
    save_settings: ({ settings }) => store.set((s) => ({ ...s, settings })),
    send_wish: ({ text }) => {
      if (text.trim() === "") throw mockError("VEM-REPORT-CMD-01", undefined, "empty");
    },
    help_report: () =>
      [
        "Vorige Eeuw Mail 0.1.0 (proefversie in de browser)",
        "Windows: onbekend",
        "Installatienummer: 0000-PROEF",
        "Postvak 1: imap.solcon.nl:993 tls",
        "Postvak 2: imap.gmail.com:993 tls",
      ].join("\n"),
    // The mock pretends the phone scans the code at once.
    pair_phone: () => {
      store.set((s) => ({ ...s, phonePaired: true }));
      return { qrSvg: SAMPLE_QR };
    },
    // The phone side: anything that is not JSON is not our code.
    pair_with_code: ({ text }) => {
      if (!text.trim().startsWith("{")) throw mockError("VEM-PAIR-READ-01");
      store.set((s) => ({ ...s, phonePaired: true }));
    },
    phone_paired: () => store.get().phonePaired,
    phone_last_copy: () => null,
    unpair_phone: () => store.set((s) => ({ ...s, phonePaired: false })),
    quit_app: () => undefined,
  };
}
