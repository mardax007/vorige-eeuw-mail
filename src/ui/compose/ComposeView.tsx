import { useCallback, useState } from "react";
import { api } from "../../api/client";
import { toVemError } from "../../api/errors";
import type { Draft, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { ErrorBox } from "../common/ErrorBox";
import { fileName } from "../format";
import { mailAccounts } from "../folders";
import { useApp } from "../state/AppContext";
import { AddressField } from "./AddressField";
import { formatAddressList, parseAddressList } from "./address";
import { AUTOSAVE_MS, useAutosave } from "./useAutosave";

/** More recipients than this asks once before sending (docs/screens.md). */
export const MANY_RECIPIENTS = 10;

interface ComposeViewProps {
  initial: Draft;
  autosaveMs?: number;
  onClose: () => void;
}

interface Fields {
  accountId: string;
  to: string;
  cc: string;
  subject: string;
  body: string;
  attachments: string[];
  shrinkPhotos: boolean;
}

function fieldsOf(d: Draft): Fields {
  return {
    accountId: d.accountId,
    to: formatAddressList(d.to),
    cc: formatAddressList(d.cc),
    subject: d.subject,
    body: d.bodyText,
    attachments: d.attachments,
    shrinkPhotos: d.shrinkPhotos,
  };
}

/** "Nieuwe e-mail": write, attach, send. The text is kept in Concepten all along. */
export function ComposeView({ initial, autosaveMs = AUTOSAVE_MS, onClose }: ComposeViewProps) {
  const { accounts, startSend, go } = useApp();
  const [fields, setFields] = useState<Fields>(() => fieldsOf(initial));
  const [draftId, setDraftId] = useState<string | null>(initial.id ?? null);
  const [version, setVersion] = useState(0);
  const [isCcShown, setIsCcShown] = useState(initial.cc.length > 0);
  const [problem, setProblem] = useState<string | null>(null);
  const [error, setError] = useState<VemError | null>(null);
  const [savedNote, setSavedNote] = useState(false);
  const [isConfirming, setIsConfirming] = useState(false);
  const mail = mailAccounts(accounts);

  const update = (patch: Partial<Fields>) => {
    setFields((f) => ({ ...f, ...patch }));
    setVersion((v) => v + 1);
    setSavedNote(false);
  };

  const buildDraft = useCallback(
    (id: string | null): Draft => ({
      ...initial,
      id,
      accountId: fields.accountId,
      to: parseAddressList(fields.to).addresses,
      cc: parseAddressList(fields.cc).addresses,
      subject: fields.subject,
      bodyText: fields.body,
      attachments: fields.attachments,
      shrinkPhotos: fields.shrinkPhotos,
    }),
    [fields, initial],
  );

  const hasContent =
    fields.to.trim() !== "" || fields.cc.trim() !== "" || fields.subject.trim() !== "" ||
    fields.body.trim() !== initial.bodyText.trim() || fields.attachments.length > 0;

  const save = useCallback(async (): Promise<boolean> => {
    try {
      setDraftId(await api.saveDraft(buildDraft(draftId)));
      setSavedNote(true);
      return true;
    } catch (e: unknown) {
      setError(toVemError(e));
      return false;
    }
  }, [buildDraft, draftId]);

  const autosave = useAutosave({ version, hasContent, intervalMs: autosaveMs, save });

  const handleClose = async () => {
    await autosave.flush();
    onClose();
  };

  const addAttachments = async () => {
    try {
      const paths = await api.pickAttachments();
      if (paths.length > 0) update({ attachments: [...fields.attachments, ...paths] });
    } catch (e: unknown) {
      setError(toVemError(e));
    }
  };

  const send = async () => {
    setIsConfirming(false);
    try {
      const item = await api.sendMessage(buildDraft(draftId));
      autosave.stop();
      startSend(item);
      go({ kind: "mail" });
    } catch (e: unknown) {
      setError(toVemError(e));
    }
  };

  const handleSend = () => {
    const to = parseAddressList(fields.to);
    const cc = parseAddressList(fields.cc);
    const invalid = [...to.invalid, ...cc.invalid];
    setProblem(null);
    if (invalid.length > 0) return setProblem(t("compose.badAddress", { address: invalid[0] }));
    if (to.addresses.length === 0) return setProblem(t("compose.needTo"));
    if (to.addresses.length + cc.addresses.length > MANY_RECIPIENTS) return setIsConfirming(true);
    void send();
  };

  const recipientCount = parseAddressList(fields.to).addresses.length + parseAddressList(fields.cc).addresses.length;

  return (
    <section className="compose" aria-label={t("compose.title")}>
      <header className="compose-header">
        <h2>{t("compose.title")}</h2>
        <div className="button-row">
          <button type="button" className="primary" onClick={handleSend}>{t("compose.send")}</button>
          <button type="button" onClick={handleClose}>{t("compose.close")}</button>
        </div>
      </header>
      {savedNote && <p className="calm-line" role="status">{t("compose.saved")}</p>}
      {problem && <p className="form-problem" role="alert">{problem}</p>}
      {error && <ErrorBox error={error} onClose={() => setError(null)} />}
      {mail.length > 1 && (
        <div className="form-row">
          <label htmlFor="compose-from">{t("compose.from")}</label>
          <select id="compose-from" value={fields.accountId} onChange={(e) => update({ accountId: e.target.value })}>
            {mail.map((a) => (
              <option key={a.id} value={a.id}>{`${a.displayName} <${a.email}>`}</option>
            ))}
          </select>
        </div>
      )}
      <AddressField id="compose-to" label={t("compose.to")} value={fields.to} onChange={(to) => update({ to })} />
      {isCcShown ? (
        <AddressField id="compose-cc" label={t("compose.cc")} value={fields.cc} onChange={(cc) => update({ cc })} />
      ) : (
        <button type="button" className="small-button" onClick={() => setIsCcShown(true)}>{t("compose.addCc")}</button>
      )}
      <div className="form-row">
        <label htmlFor="compose-subject">{t("compose.subject")}</label>
        <input id="compose-subject" type="text" value={fields.subject} onChange={(e) => update({ subject: e.target.value })} />
      </div>
      <div className="form-row grow">
        <label htmlFor="compose-body">{t("compose.body")}</label>
        <textarea id="compose-body" value={fields.body} rows={14} onChange={(e) => update({ body: e.target.value })} />
      </div>
      <div className="compose-attachments">
        <button type="button" onClick={addAttachments}>{t("compose.attach")}</button>
        <label className="check-row">
          <input type="checkbox" checked={fields.shrinkPhotos} onChange={(e) => update({ shrinkPhotos: e.target.checked })} />
          {t("compose.shrinkPhotos")}
        </label>
        {fields.attachments.length > 0 && (
          <ul>
            {fields.attachments.map((path, i) => (
              <li key={`${path}-${i}`} className="attachment-row">
                <span>{fileName(path)}</span>
                <button type="button" onClick={() => update({ attachments: fields.attachments.filter((_, j) => j !== i) })}>
                  {t("compose.removeAttachment")}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {isConfirming && (
        <ConfirmDialog
          text={t("compose.manyConfirm", { count: recipientCount })}
          confirmLabel={t("compose.manyYes")}
          onConfirm={() => void send()}
          onCancel={() => setIsConfirming(false)}
        />
      )}
    </section>
  );
}
