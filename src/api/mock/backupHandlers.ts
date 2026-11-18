import type { CommandHandlers } from "../commands";
import type { BackupInfo, BackupProgress } from "../types";
import type { MockStore } from "./state";
import { mockError } from "./state";

type BackupCommands = "start_backup" | "stop_backup" | "pick_backup" | "start_restore";

const TOTAL_BYTES = 1_800_000_000;
const STEPS = 6;
const STEP_MS = 150;

/** The made-up backup the mock finds on the made-up USB stick. */
export const SAMPLE_BACKUP: BackupInfo = {
  path: "E:\\Vorige Eeuw Mail reservekopie 2026-11-01",
  createdAt: "2026-11-01T10:00:00Z",
  accounts: [
    { name: "Ria Jansen", email: "ria.jansen@solcon.nl" },
    { name: "Ria Jansen (Gmail)", email: "ria.jansen1950@gmail.com" },
  ],
  messages: 3210,
  bytes: TOTAL_BYTES,
};

/** Backup and restore pretend to copy in a few steps; the mock never restarts. */
export function createBackupHandlers(store: MockStore): Pick<CommandHandlers, BackupCommands> {
  let running = false;
  let stopped = false;

  const run = (restoring: boolean, last: Partial<BackupProgress>) => {
    if (running) throw mockError("VEM-APP-CMD-01", undefined, "a backup, restore or import is running");
    running = true;
    stopped = false;
    const emit = (p: Partial<BackupProgress>) =>
      store.events.emit("backup:progress", { restoring, doneBytes: 0, totalBytes: TOTAL_BYTES, finished: false, stopped: false, ...p });
    const step = (n: number) => {
      if (stopped) {
        running = false;
        emit({ finished: true, stopped: true });
        return;
      }
      if (n > STEPS) {
        running = false;
        emit({ doneBytes: TOTAL_BYTES, finished: true, ...last });
        return;
      }
      emit({ doneBytes: Math.round((TOTAL_BYTES * n) / STEPS) });
      store.later(STEP_MS, () => step(n + 1));
    };
    store.later(STEP_MS, () => step(0));
  };

  return {
    start_backup: () => {
      run(false, { folder: "E:\\Vorige Eeuw Mail reservekopie 2026-11-18" });
      return true;
    },
    stop_backup: () => {
      stopped = true;
    },
    pick_backup: () => SAMPLE_BACKUP,
    start_restore: () => run(true, {}),
  };
}
