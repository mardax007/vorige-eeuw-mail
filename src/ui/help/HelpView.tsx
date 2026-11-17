import { useState } from "react";
import { api } from "../../api/client";
import { toVemError } from "../../api/errors";
import type { VemError } from "../../api/types";
import { t } from "../../i18n";
import { CopyNoteText, ErrorBox } from "../common/ErrorBox";
import { useCopyNote } from "../common/useCopyNote";
import { useApp } from "../state/AppContext";

/** Help: a wish to Martijn, the help report, and the note about help from afar. */
export function HelpView() {
  const { showError, appInfo } = useApp();
  const isPhone = appInfo?.isPhone === true;
  const [wish, setWish] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [isSent, setIsSent] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [wishError, setWishError] = useState<VemError | null>(null);
  const reportNote = useCopyNote();

  const sendWish = async () => {
    setProblem(null);
    setWishError(null);
    setIsSent(false);
    if (wish.trim() === "") {
      setProblem(t("help.wishEmpty"));
      return;
    }
    setIsSending(true);
    try {
      await api.sendWish(wish.trim());
      setWish("");
      setIsSent(true);
    } catch (e: unknown) {
      setWishError(toVemError(e));
    } finally {
      setIsSending(false);
    }
  };

  // The report could not be made: the note says copying failed, and the error says why.
  const copyReport = () => reportNote.copy(() => api.helpReport(), showError);

  return (
    <section className="page help">
      <h2>{t("help.title")}</h2>

      <h3>{t("help.wish")}</h3>
      <div className="form-row">
        <label htmlFor="wish-text">{t("help.wishText")}</label>
        <textarea
          id="wish-text"
          rows={5}
          value={wish}
          aria-invalid={problem ? true : undefined}
          aria-describedby={problem ? "wish-problem" : undefined}
          onChange={(e) => {
            setWish(e.target.value);
            setProblem(null);
            setIsSent(false);
          }}
        />
        {problem && (
          <p className="form-problem" id="wish-problem" role="alert">
            {problem}
          </p>
        )}
      </div>
      {wishError && <ErrorBox error={wishError} onClose={() => setWishError(null)} />}
      <div className="button-row">
        <button type="button" className="primary" onClick={sendWish} disabled={isSending}>
          {isSending ? t("general.saving") : t("help.wishSend")}
        </button>
        <span role="status">{isSent ? t("help.wishSent") : ""}</span>
      </div>

      <h3>{t("help.report")}</h3>
      <p>{isPhone ? t("help.reportTextPhone") : t("help.reportText")}</p>
      <div className="button-row">
        <button type="button" onClick={copyReport}>{t("help.report")}</button>
        <CopyNoteText result={reportNote.result} copiedText={t("help.reportCopied")} />
      </div>

      {/* Snelle hulp is a Windows program; a phone has nothing like it. */}
      {!isPhone && (
        <>
          <h3>{t("help.remote")}</h3>
          <p>{t("help.remoteText")}</p>
        </>
      )}

      <h3>{t("help.noticeTitle")}</h3>
      <p>{t("help.notice")}</p>
    </section>
  );
}
