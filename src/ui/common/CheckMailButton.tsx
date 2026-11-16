import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

/** How long "De mail is opgehaald." stays next to the button. */
export const CHECKED_NOTE_MS = 4000;
/** If the core never says it is busy, the button is free again after this long (nothing ran). */
export const CHECK_GIVE_UP_MS = 15_000;
/** If the core never says it is done, the button is free again after this long. */
export const CHECK_BUSY_GIVE_UP_MS = 120_000;

type Phase = "idle" | "asked" | "busy" | "done";

/** "Mail ophalen": shows that it is working, and says when the mail is in. */
export function CheckMailButton() {
  const { isSyncing, syncErrors, showError } = useApp();
  const hasSyncError = Object.keys(syncErrors).length > 0;
  const [phase, setPhase] = useState<Phase>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // sync_now only wakes the core; the sync:state events tell when the fetching starts and ends.
  useEffect(() => {
    if (phase === "asked" && isSyncing) setPhase("busy");
    // Ended with an error: that error is on screen already, so no "opgehaald".
    if (phase === "busy" && !isSyncing) setPhase(hasSyncError ? "idle" : "done");
  }, [phase, isSyncing, hasSyncError]);

  useEffect(() => {
    if (timer.current) clearTimeout(timer.current);
    if (phase === "asked") timer.current = setTimeout(() => setPhase("idle"), CHECK_GIVE_UP_MS);
    if (phase === "busy") timer.current = setTimeout(() => setPhase("idle"), CHECK_BUSY_GIVE_UP_MS);
    if (phase === "done") timer.current = setTimeout(() => setPhase("idle"), CHECKED_NOTE_MS);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [phase]);

  const check = async () => {
    setPhase(isSyncing ? "busy" : "asked");
    try {
      await api.syncNow(null);
    } catch (e: unknown) {
      setPhase("idle");
      showError(e);
    }
  };

  const isWorking = phase === "asked" || phase === "busy";
  return (
    <>
      <button type="button" disabled={isWorking} onClick={() => void check()}>
        {isWorking ? t("nav.checkingMail") : t("nav.checkMail")}
      </button>
      <span className="check-note" role="status">
        {phase === "done" ? t("nav.mailChecked") : ""}
      </span>
    </>
  );
}
