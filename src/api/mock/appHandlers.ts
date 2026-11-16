import type { CommandHandlers } from "../commands";
import type { Account, ServerSettings } from "../types";
import type { MockFolder, MockStore } from "./state";
import { mockError } from "./state";

type AppCommands =
  | "app_info"
  | "list_accounts"
  | "lookup_settings"
  | "add_account"
  | "remove_account"
  | "set_signature"
  | "sync_now"
  | "get_settings"
  | "save_settings"
  | "find_import_sources"
  | "pick_import_file"
  | "start_import"
  | "list_originals"
  | "delete_original"
  | "export_folder"
  | "send_wish"
  | "help_report"
  | "pair_phone"
  | "unpair_phone"
  | "quit_app";

/** A made-up QR-like picture; the real one comes from the core. */
const SAMPLE_QR =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 21 21" shape-rendering="crispEdges">' +
  '<rect width="21" height="21" fill="#fff"/>' +
  '<path fill="#000" d="M0 0h7v7H0zM14 0h7v7h-7zM0 14h7v7H0zM9 2h2v2H9zM9 9h3v3H9zM14 14h2v2h-2zM17 17h3v3h-3zM12 15h1v4h-1z"/>' +
  '<path fill="#fff" d="M1 1h5v5H1zM15 1h5v5h-5zM1 15h5v5H1z"/>' +
  '<path fill="#000" d="M2 2h3v3H2zM16 2h3v3h-3zM2 16h3v3H2z"/></svg>';

function settingsFor(email: string): ServerSettings {
  const domain = email.split("@")[1]?.toLowerCase() ?? "";
  if (domain === "" || domain.startsWith("onbekend")) {
    throw mockError("VEM-LOOKUP-FIND-01", undefined, domain);
  }
  const known = domain === "solcon.nl" || domain === "gmail.com";
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
    signIn: domain === "gmail.com" ? "google" : "password",
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

function runImport(store: MockStore, jobId: string, label: string, size: number): void {
  const total = 500;
  const step = (done: number) => {
    const finished = done >= total;
    store.events.emit("import:progress", {
      jobId,
      done: Math.min(done, total),
      total,
      currentFolder: finished ? null : done < 250 ? "Postvak IN" : "Verzonden items",
      finished,
      error: null,
    });
    if (finished) {
      store.set((s) => ({
        ...s,
        originals: [...s.originals, { id: store.nextId("orig"), label, size, importedAt: new Date().toISOString() }],
      }));
      return;
    }
    store.later(200, () => step(done + 50));
  };
  store.later(100, () => step(0));
}

export function createAppHandlers(store: MockStore): Pick<CommandHandlers, AppCommands> {
  return {
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
      if (password === "fout") {
        throw mockError("VEM-IMAP-LOGIN-01", "a1 NO [AUTHENTICATIONFAILED] Authentication failed.");
      }
      const account: Account = {
        id: store.nextId("acc"),
        displayName,
        email,
        kind: "imap",
        quota: { usedKb: 120_000, limitKb: 1_000_000 },
        signature: null,
        lastError: null,
      };
      store.set((s) => ({
        ...s,
        accounts: [...s.accounts, account],
        folders: [...s.folders, ...standardFolders(account.id)],
      }));
      return account;
    },
    remove_account: ({ accountId }) =>
      store.set((s) => ({
        ...s,
        accounts: s.accounts.filter((a) => a.id !== accountId),
        folders: s.folders.filter((f) => f.accountId !== accountId),
        messages: s.messages.filter((m) => m.accountId !== accountId),
      })),
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
    find_import_sources: () => [
      {
        kind: "pst",
        path: "C:\\Users\\Ria\\Documents\\Outlook-bestanden\\Outlook.pst",
        size: 1_288_490_188,
        label: "Outlook.pst",
      },
      { kind: "wlm", path: "C:\\Users\\Ria\\AppData\\Local\\Microsoft\\Windows Live Mail", size: null, label: "Windows Live Mail" },
    ],
    pick_import_file: ({ kind }) => ({
      kind,
      path: `C:\\Users\\Ria\\Documents\\oud.${kind}`,
      size: 52_428_800,
      label: `oud.${kind}`,
    }),
    start_import: ({ source }) => {
      const jobId = store.nextId("job");
      runImport(store, jobId, source.label, source.size ?? 0);
      return jobId;
    },
    list_originals: () => store.get().originals,
    delete_original: ({ id }) =>
      store.set((s) => ({ ...s, originals: s.originals.filter((o) => o.id !== id) })),
    export_folder: ({ folderId }) => {
      const folder = store.get().folders.find((f) => f.id === folderId);
      return `C:\\Users\\Ria\\Documents\\${folder?.path.join(" - ") ?? "map"}.mbox`;
    },
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
    pair_phone: () => ({ qrSvg: SAMPLE_QR }),
    unpair_phone: () => undefined,
    quit_app: () => undefined,
  };
}
