import type { VemError } from "../../api/types";
import { errorText, t } from "../../i18n";
import { useCopyNote, type CopyResult } from "./useCopyNote";

interface ErrorBoxProps {
  error: VemError;
  /** Says Martijn hears of it; by default when the core queued a report about this error. */
  isReported?: boolean;
  onClose?: () => void;
}

/** The text "Kopiëren voor Martijn" puts on the clipboard: sentence, exact reply, code. */
export function textForMartijn(error: VemError): string {
  const { sentence, next } = errorText(error.code);
  const raw = error.serverReply ?? error.detail;
  const label = error.serverReply ? t("error.serverSaid") : t("error.details");
  const lines = [sentence, next];
  if (raw) lines.push("", label, raw);
  lines.push("", t("error.code", { code: error.code }));
  return lines.join("\n");
}

interface CopyNoteTextProps {
  result: CopyResult | null;
  /** Said when copying worked; "Gekopieerd." unless given. */
  copiedText?: string;
}

/** The note next to one copy button: copied, or how to copy by hand. Empty otherwise. */
export function CopyNoteText({ result, copiedText = t("error.copied") }: CopyNoteTextProps) {
  const text = result === "copied" ? copiedText : result === "failed" ? t("error.copyFailed") : "";
  return (
    <span className="copy-note" role="status">
      {text}
    </span>
  );
}

/** One error, as docs/errors.md says: what happened, what to do, the exact words, the code. */
export function ErrorBox({ error, isReported = error.reported === true, onClose }: ErrorBoxProps) {
  const rawNote = useCopyNote();
  const martijnNote = useCopyNote();
  const { sentence, next } = errorText(error.code);
  const raw = error.serverReply ?? error.detail ?? null;
  const label = error.serverReply ? t("error.serverSaid") : t("error.details");

  return (
    <section className="error-box" role="alert">
      <p className="error-sentence">{sentence}</p>
      <p>{next}</p>
      {raw !== null && raw !== "" && (
        <div className="error-raw">
          <p className="error-label">{label}</p>
          <pre className="error-reply">{raw}</pre>
          <div className="button-row">
            <button type="button" onClick={() => void rawNote.copy(raw)}>
              {t("error.copy")}
            </button>
            <CopyNoteText result={rawNote.result} />
          </div>
        </div>
      )}
      <p className="error-code">{t("error.code", { code: error.code })}</p>
      {isReported && <p>{t("error.reported")}</p>}
      <div className="button-row">
        <button type="button" onClick={() => void martijnNote.copy(textForMartijn(error))}>
          {t("error.copyForMartijn")}
        </button>
        <CopyNoteText result={martijnNote.result} />
        {onClose && (
          <button type="button" onClick={onClose}>
            {t("error.close")}
          </button>
        )}
      </div>
    </section>
  );
}
