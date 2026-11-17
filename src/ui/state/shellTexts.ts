import { api } from "../../api/client";
import type { ShellTexts } from "../../api/types";
import { t } from "../../i18n";

/** The Dutch texts the shell shows itself. {count} stays in for the shell to fill. */
export function shellTexts(): ShellTexts {
  return {
    open: t("shell.open"),
    quit: t("shell.quit"),
    newMail: t("shell.newMail"),
    newMailMany: t("shell.newMailMany"),
    stillRunning: t("shell.stillRunning"),
    watching: t("shell.watching"),
    watchingChannel: t("shell.watchingChannel"),
    newMailChannel: t("shell.newMailChannel"),
  };
}

/** Hands the texts to the shell once at start. */
export function sendShellTexts(): void {
  // Cosmetic only: if this fails the shell keeps its built-in texts, so the user is not bothered.
  api.setShellTexts(shellTexts()).catch(() => undefined);
}
