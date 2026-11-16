import { useState } from "react";
import type { VemError } from "../../api/types";
import { errorText, t } from "../../i18n";
import { copyText } from "./clipboard";

interface ErrorBoxProps {
  error: VemError;
  /** True once reports to Martijn are built and this error was reported. */
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

/** One error, as docs/errors.md says: what happened, what to do, the exact words, the code. */
export function ErrorBox({ error, isReported = false, onClose }: ErrorBoxProps) {
  const [copied, setCopied] = useState(false);
  const { sentence, next } = errorText(error.code);
  const raw = error.serverReply ?? error.detail ?? null;
  const label = error.serverReply ? t("error.serverSaid") : t("error.details");

  const handleCopy = async (text: string) => {
    setCopied(await copyText(text));
  };

  return (
    <section className="error-box" role="alert">
      <p className="error-sentence">{sentence}</p>
      <p>{next}</p>
      {raw !== null && raw !== "" && (
        <div className="error-raw">
          <p className="error-label">{label}</p>
          <pre className="error-reply">{raw}</pre>
          <button type="button" onClick={() => handleCopy(raw)}>
            {t("error.copy")}
          </button>
        </div>
      )}
      <p className="error-code">{t("error.code", { code: error.code })}</p>
      {isReported && <p>{t("error.reported")}</p>}
      <div className="button-row">
        <button type="button" onClick={() => handleCopy(textForMartijn(error))}>
          {t("error.copyForMartijn")}
        </button>
        {onClose && (
          <button type="button" onClick={onClose}>
            {t("error.close")}
          </button>
        )}
        {copied && <span role="status">{t("error.copied")}</span>}
      </div>
    </section>
  );
}
