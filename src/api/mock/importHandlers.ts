import type { CommandHandlers } from "../commands";
import type { ImportProgress, ImportSkip, VemError } from "../types";
import type { MockStore } from "./state";
import { mockError } from "./state";

type ImportCommands =
  | "find_import_sources"
  | "pick_import_file"
  | "import_contacts_file"
  | "start_import"
  | "rescue_available"
  | "import_from_account"
  | "import_status"
  | "list_originals"
  | "delete_original"
  | "export_folder";

const IMPORT_TOTAL = 500;
const IMPORT_STEP = 50;
const IMPORT_STEP_MS = 200;
const IMPORT_START_MS = 100;

interface ImportRun {
  /** Folders the mock pretends to pass through. */
  folders: readonly [string, string];
  alreadyThere: number;
  contactsAdded?: number;
  allMarkedRead?: boolean;
  rescued?: boolean;
  /** The import stops at once with this error (a damaged file). */
  error?: VemError;
  skipped: ImportSkip[];
  /** The file or mailbox, as the screen named it. */
  source: string;
  /** The folder the new mail went into. */
  folderId?: string;
  /** Set for a file import: the kept copy that shows under Originele bestanden. */
  original?: { label: string; size: number };
}

function report(store: MockStore, progress: ImportProgress): void {
  store.set((s) => ({ ...s, lastImport: progress }));
  store.events.emit("import:progress", progress);
}

function runImport(store: MockStore, jobId: string, run: ImportRun): void {
  if (run.error) {
    const error = run.error;
    store.later(IMPORT_START_MS, () =>
      report(store, { jobId, done: 0, total: null, currentFolder: null, finished: true, error, alreadyThere: 0, skipped: [], source: run.source }),
    );
    return;
  }
  const step = (done: number) => {
    const finished = done >= IMPORT_TOTAL;
    report(store, {
      jobId,
      done: Math.min(done, IMPORT_TOTAL),
      total: IMPORT_TOTAL,
      currentFolder: finished ? null : done < IMPORT_TOTAL / 2 ? run.folders[0] : run.folders[1],
      finished,
      error: null,
      alreadyThere: finished ? run.alreadyThere : 0,
      contactsAdded: finished ? (run.contactsAdded ?? 0) : 0,
      allMarkedRead: finished && (run.allMarkedRead ?? false),
      rescued: finished && (run.rescued ?? false),
      skipped: finished ? run.skipped : [],
      source: run.source,
      folderId: finished ? (run.folderId ?? null) : null,
    });
    if (finished) {
      const original = run.original;
      if (original) {
        store.set((s) => ({
          ...s,
          originals: [
            ...s.originals,
            { id: store.nextId("orig"), label: original.label, size: original.size, importedAt: new Date().toISOString() },
          ],
        }));
      }
      return;
    }
    store.later(IMPORT_STEP_MS, () => step(done + IMPORT_STEP));
  };
  store.later(IMPORT_START_MS, () => step(0));
}

/** One import at a time, as in the core. */
function startJob(store: MockStore, run: ImportRun): string {
  const last = store.get().lastImport;
  if (last && !last.finished) throw mockError("VEM-APP-CMD-01", undefined, "an import is already running");
  const jobId = store.nextId("job");
  report(store, { jobId, done: 0, total: null, currentFolder: null, finished: false, error: null, alreadyThere: 0, skipped: [], source: run.source });
  runImport(store, jobId, run);
  return jobId;
}

export function createImportHandlers(store: MockStore): Pick<CommandHandlers, ImportCommands> {
  return {
    find_import_sources: () => [
      {
        kind: "pst",
        path: "C:\\Users\\Ria\\Documents\\Outlook-bestanden\\Outlook.pst",
        size: 1_288_490_188,
        label: "Outlook.pst",
      },
      {
        kind: "pst",
        path: "C:\Users\Ria\Documents\Outlook-bestanden\Archief 2012.pst",
        size: 734_003_200,
        label: "Archief 2012.pst",
        protected: true,
      },
      { kind: "wlm", path: "C:\\Users\\Ria\\AppData\\Local\\Microsoft\\Windows Live Mail", size: null, label: "Windows Live Mail" },
    ],
    import_contacts_file: () => {
      const known = store.get().contacts.some((c) => c.emails.includes("kees.jansen@voorbeeld.nl"));
      store.set((s) => ({
        ...s,
        contacts: [
          ...s.contacts,
          { id: store.nextId("c"), name: "Truus de Wit", emails: ["truus.dewit@voorbeeld.nl"], phone: "06-12345678", note: null },
        ],
      }));
      return { found: 2, added: known ? 1 : 2 };
    },
    pick_import_file: ({ kind }) => ({
      kind,
      path: `C:\\Users\\Ria\\Documents\\oud.${kind}`,
      size: 52_428_800,
      label: `oud.${kind}`,
    }),
    rescue_available: () => true,
    start_import: ({ source, password, rescue }) => {
      // The mock's protected file opens with "geheim" (made-up test value).
      if (source.protected && password !== "geheim") {
        throw mockError("VEM-IMPORT-READ-05", undefined, password ? "wrong" : "missing");
      }
      // The mock's damaged file: the normal reader stops, the rescue tool reads it.
      const damaged = source.label === "Kapot.pst" && !rescue;
      return startJob(store, {
        error: damaged ? mockError("VEM-IMPORT-READ-01", undefined, "PST cannot be read: invalid block") : undefined,
        source: source.label,
        folderId: "f-l-inbox",
        rescued: rescue,
        folders: ["Postvak IN", "Verzonden items"],
        alreadyThere: 0,
        contactsAdded: source.kind === "pst" ? 3 : 0,
        allMarkedRead: source.kind === "wlm",
        skipped: [{ folder: "Agenda", reason: "not a mail folder (IPF.Appointment)" }],
        original: { label: source.label, size: source.size ?? 0 },
      });
    },
    import_from_account: ({ sourceAccountId, target }) => {
      const accounts = store.get().accounts;
      if (!accounts.some((a) => a.id === sourceAccountId)) {
        throw mockError("VEM-STORE-READ-01", undefined, `account ${sourceAccountId}`);
      }
      if (target.kind === "mailbox" && target.accountId === sourceAccountId) {
        throw mockError("VEM-APP-CMD-01", undefined, "source and target are the same mailbox");
      }
      return startJob(store, {
        source: accounts.find((a) => a.id === sourceAccountId)?.displayName ?? "",
        folders: ["INBOX", "Sent"],
        alreadyThere: 12,
        skipped: [{ folder: "Archief/2009", reason: "SELECT failed: NO Mailbox is locked" }],
      });
    },
    import_status: () => store.get().lastImport,
    list_originals: () => store.get().originals,
    delete_original: ({ id }) =>
      store.set((s) => ({ ...s, originals: s.originals.filter((o) => o.id !== id) })),
    export_folder: ({ folderId }) => {
      const folder = store.get().folders.find((f) => f.id === folderId);
      return `C:\\Users\\Ria\\Documents\\${folder?.path.join(" - ") ?? "map"}.mbox`;
    },
  };
}
