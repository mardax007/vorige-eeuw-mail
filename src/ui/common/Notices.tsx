import { useEffect, useState } from "react";
import { api } from "../../api/client";
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

/** "Wordt verstuurd over N seconden" with "Ongedaan maken" (C3). */
export function SendBar() {
  const { pendingSend, clearSend, compose, showError, outbox } = useApp();
  const [left, setLeft] = useState(() => (pendingSend ? secondsLeft(pendingSend.sendAt) : 0));

  useEffect(() => {
    if (!pendingSend) return;
    setLeft(secondsLeft(pendingSend.sendAt));
    const timer = setInterval(() => setLeft(secondsLeft(pendingSend.sendAt)), 500);
    return () => clearInterval(timer);
  }, [pendingSend]);

  const stillWaiting = pendingSend !== null && outbox.some((o) => o.id === pendingSend.id && o.state === "waiting");

  useEffect(() => {
    if (pendingSend && left === 0 && !stillWaiting) {
      const timer = setTimeout(clearSend, 3000);
      return () => clearTimeout(timer);
    }
  }, [pendingSend, left, stillWaiting, clearSend]);

  if (!pendingSend) return null;

  const handleUndo = async () => {
    try {
      const draft = await api.cancelSend(pendingSend.id);
      clearSend();
      compose(draft);
    } catch (e: unknown) {
      clearSend();
      showError(e);
    }
  };

  const text =
    left > 0 ? t("compose.sending", { seconds: left }) : stillWaiting ? t("compose.sendingNow") : t("compose.sent");

  return (
    <div className="notice-bar" role="status">
      <span>{text}</span>
      {left > 0 && (
        <button type="button" className="primary" onClick={handleUndo}>
          {t("undo.button")}
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
