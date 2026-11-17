import { useEffect, useRef, useState } from "react";
import { api, onEvent } from "../../api/client";
import type { OutboxItem, UpdateOffer } from "../../api/types";
import { t } from "../../i18n";
import { useApp } from "../state/AppContext";

/** The "Ongedaan maken" bar after Verwijderen or Verplaatsen. */
export function UndoBar() {
  const { undoNotice, dismissUndo, showError } = useApp();
  if (!undoNotice) return null;

  const handleUndo = async () => {
    dismissUndo();
    try {
      await undoNotice.onUndo();
    } catch (e: unknown) {
      showError(e);
    }
  };

  return (
    <div className="notice-bar" role="status">
      <span>{undoNotice.text}</span>
      <button type="button" className="primary" onClick={handleUndo}>
        {t("undo.button")}
      </button>
    </div>
  );
}

function secondsLeft(sendAt: string): number {
  return Math.max(0, Math.ceil((new Date(sendAt).getTime() - Date.now()) / 1000));
}

/** How long "Het bericht is verstuurd." stays once the message really went. */
export const SENT_BAR_MS = 4000;

export type SendPhase = "countdown" | "sending" | "failed" | "sent";

/**
 * What the send bar says, from the outbox item itself. A message counts as sent only when it was
 * in Postvak UIT and is gone from it; before it was ever seen there, it is still being sent.
 */
export function sendPhase(item: OutboxItem | undefined, wasSeen: boolean, left: number): SendPhase {
  if (item) {
    if (item.state === "failed") return "failed";
    return item.state === "waiting" && left > 0 ? "countdown" : "sending";
  }
  if (wasSeen) return "sent";
  return left > 0 ? "countdown" : "sending";
}

/** "Wordt verstuurd over N seconden" with "Ongedaan maken" (C3), then what really happened. */
export function SendBar() {
  const { pendingSend, clearSend, compose, showError, outbox, setLocation, go } = useApp();
  // Ticks twice a second to count down; the seconds themselves come from the clock on every render.
  const [, setTick] = useState(0);
  const left = pendingSend ? secondsLeft(pendingSend.sendAt) : 0;
  // The send whose item was really found in Postvak UIT; only then does its absence mean "sent".
  const [seenId, setSeenId] = useState<string | null>(null);
  const [isUndoing, setIsUndoing] = useState(false);
  const isUndoingRef = useRef(false);

  const isCounting = left > 0;
  useEffect(() => {
    if (!isCounting) return;
    const timer = setInterval(() => setTick((n) => n + 1), 500);
    return () => clearInterval(timer);
  }, [isCounting, pendingSend]);

  const item = pendingSend ? outbox.find((o) => o.id === pendingSend.id) : undefined;
  const pendingId = pendingSend?.id ?? null;
  const wasSeen = pendingId !== null && seenId === pendingId;
  const phase = sendPhase(item, wasSeen, left);

  useEffect(() => {
    if (item) setSeenId(item.id);
  }, [item]);

  // The wait is over and the item was never in the list on screen (it may have come and gone between
  // two updates): read Postvak UIT once. In it means seen; not in it, after Versturen put it there, means sent.
  const isUnseenAfterWait = pendingId !== null && !item && !wasSeen && left === 0;
  useEffect(() => {
    if (!isUnseenAfterWait || pendingId === null) return;
    let alive = true;
    api
      .listOutbox()
      .then((list) => {
        if (alive && !list.some((o) => o.id === pendingId)) setSeenId(pendingId);
      })
      // The list did not load: the bar keeps saying it is being sent; the next update decides.
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [isUnseenAfterWait, pendingId]);

  useEffect(() => {
    if (pendingSend && phase === "sent") {
      const timer = setTimeout(clearSend, SENT_BAR_MS);
      return () => clearTimeout(timer);
    }
  }, [pendingSend, phase, clearSend]);

  if (!pendingSend) return null;

  const handleUndo = async () => {
    // One undo per send: a second click would find nothing left to take back and show an error.
    if (isUndoingRef.current) return;
    isUndoingRef.current = true;
    setIsUndoing(true);
    try {
      const draft = await api.cancelSend(pendingSend.id);
      clearSend();
      compose(draft);
    } catch (e: unknown) {
      clearSend();
      showError(e);
    } finally {
      isUndoingRef.current = false;
      setIsUndoing(false);
    }
  };

  const openOutbox = () => {
    clearSend();
    setLocation({ kind: "outbox" });
    go({ kind: "mail" });
  };

  const texts: Record<SendPhase, string> = {
    countdown: t("compose.sending", { seconds: left }),
    sending: t("compose.sendingNow"),
    failed: t("compose.notSent"),
    sent: t("compose.sent"),
  };

  return (
    <div className={`notice-bar${phase === "failed" ? " failed" : ""}`} role="status">
      <span>{texts[phase]}</span>
      {phase === "countdown" && (
        <button type="button" className="primary" disabled={isUndoing} onClick={handleUndo}>
          {t("undo.button")}
        </button>
      )}
      {phase === "failed" && (
        <button type="button" className="primary" onClick={openOutbox}>
          {t("compose.openOutbox")}
        </button>
      )}
    </div>
  );
}

/** A small line for one day after an update, with "Wat is er nieuw?". */
export function UpdateLine() {
  const { appInfo } = useApp();
  const [isOpen, setIsOpen] = useState(false);
  const [isHidden, setIsHidden] = useState(false);
  if (!appInfo?.updatedTo || isHidden) return null;
  const news = appInfo.whatsNew ?? [];

  return (
    <div className="update-line">
      <span>{t("update.line", { version: appInfo.updatedTo })}</span>
      {news.length > 0 && (
        <button type="button" className="link-button" aria-expanded={isOpen} onClick={() => setIsOpen(!isOpen)}>
          {t("update.whatsNew")}
        </button>
      )}
      <button type="button" className="link-button" onClick={() => setIsHidden(true)}>
        {t("update.hide")}
      </button>
      {isOpen && (
        <ul className="whats-new">
          {news.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

type UpdatePhase = "offer" | "fetching" | "android";

/**
 * Android: a newer version is on GitHub. "Bijwerken" fetches and checks it, then Android's own
 * install screen asks the person (decision 22). Windows never shows this; it updates itself.
 */
export function UpdateOfferBar() {
  const { showError } = useApp();
  const [offer, setOffer] = useState<UpdateOffer | null>(null);
  const [phase, setPhase] = useState<UpdatePhase>("offer");
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    api
      .updateOffer()
      .then((found) => {
        if (alive && found) setOffer(found);
      })
      .catch(showError);
    const listening = onEvent("update:available", (found) => {
      if (alive) setOffer(found);
    });
    return () => {
      alive = false;
      void listening.then((stop) => stop());
    };
  }, [showError]);

  if (!offer) return null;

  const install = async () => {
    setPhase("fetching");
    try {
      await api.installUpdate();
      setPhase("android");
    } catch (e: unknown) {
      setPhase("offer");
      showError(e);
    }
  };
  const texts: Record<UpdatePhase, string> = {
    offer: t("update.offer", { version: offer.version }),
    fetching: t("update.fetching"),
    android: t("update.androidAsks"),
  };

  return (
    <div className="notice-bar calm" role="status">
      <span>{texts[phase]}</span>
      <button type="button" className="primary" disabled={phase === "fetching"} onClick={() => void install()}>
        {t("update.install")}
      </button>
      {offer.notes.length > 0 && (
        <button type="button" className="link-button" aria-expanded={isOpen} onClick={() => setIsOpen(!isOpen)}>
          {t("update.whatsNew")}
        </button>
      )}
      {isOpen && (
        <ul className="whats-new">
          {offer.notes.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Told once: Martijn hears when something goes wrong, without mail content. */
export function ReportNotice() {
  const { settings, updateSettings, ready } = useApp();
  if (!ready || settings.reportNoticeShown) return null;
  return (
    <div className="notice-bar calm" role="note">
      <span>{t("help.notice")}</span>
      <button type="button" onClick={() => updateSettings({ reportNoticeShown: true })}>
        {t("help.noticeOk")}
      </button>
    </div>
  );
}
