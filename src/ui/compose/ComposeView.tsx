import { useCallback, useRef, useState } from "react";
import { api } from "../../api/client";
import { toVemError } from "../../api/errors";
import type { Account, Draft, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { ErrorBox } from "../common/ErrorBox";
import { useBackStep } from "../common/useBackStep";
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
  /** One of the mailbox's other addresses (decision 32); null for its main address. */
  fromAddress: string | null;
  to: string;
  cc: string;
  subject: string;
  body: string;
  attachments: string[];
  shrinkPhotos: boolean;
}

type AddressFieldName = "to" | "cc";

interface FieldProblem {
  field: AddressFieldName;
  text: string;
}

function fieldsOf(d: Draft): Fields {
  return {
    accountId: d.accountId,
    fromAddress: d.fromAddress ?? null,
    to: formatAddressList(d.to),
    cc: formatAddressList(d.cc),
    subject: d.subject,
    body: d.bodyText,
    attachments: d.attachments,
    shrinkPhotos: d.shrinkPhotos,
  };
}

interface Sender {
  value: string;
  label: string;
  accountId: string;
  fromAddress: string | null;
}

function senderValue(accountId: string, fromAddress: string | null): string {
  return fromAddress ? `${accountId} ${fromAddress}` : accountId;
}

/** "Van": every mailbox with its main address, then its other addresses (decision 32). */
function senderChoices(mail: readonly Account[]): Sender[] {
  return mail.flatMap((a) => [
    { value: senderValue(a.id, null), label: `${a.displayName} <${a.email}>`, accountId: a.id, fromAddress: null },
    ...a.addresses.map((address) => ({
      value: senderValue(a.id, address),
      label: `${a.displayName} <${address}>`,
      accountId: a.id,
      fromAddress: address,
    })),
  ]);
}

/** What is wrong with the addresses before sending, and in which field; null when all is well. */
export function addressProblem(fields: Pick<Fields, "to" | "cc">): FieldProblem | null {
  const to = parseAddressList(fields.to);
  const cc = parseAddressList(fields.cc);
  if (to.invalid.length > 0) return { field: "to", text: t("compose.badAddress", { address: to.invalid[0] }) };
  if (cc.invalid.length > 0) return { field: "cc", text: t("compose.badAddress", { address: cc.invalid[0] }) };
  if (to.addresses.length === 0) return { field: "to", text: t("compose.needTo") };
  return null;
}

/** "Nieuwe e-mail": write, attach, send. The text is kept in Concepten all along. */
export function ComposeView({ initial, autosaveMs = AUTOSAVE_MS, onClose }: ComposeViewProps) {
  const { accounts, startSend, go, showError } = useApp();
  const [fields, setFields] = useState<Fields>(() => fieldsOf(initial));
  // A ref as well as state: a save right after another must reuse the id the first one got.
  const draftId = useRef<string | null>(initial.id ?? null);
  const [version, setVersion] = useState(0);
  const [isCcShown, setIsCcShown] = useState(initial.cc.length > 0);
  const [problem, setProblem] = useState<FieldProblem | null>(null);
  const [error, setError] = useState<VemError | null>(null);
  const [isSavedNoteShown, setIsSavedNoteShown] = useState(false);
  const [isCloseRefused, setIsCloseRefused] = useState(false);
  const [isConfirming, setIsConfirming] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const isSendingRef = useRef(false);
  const isClosing = useRef(false);
  // The last save error, for when the screen is gone before it can show it.
  const lastSaveError = useRef<VemError | null>(null);
  const toRef = useRef<HTMLInputElement>(null);
  const ccRef = useRef<HTMLInputElement>(null);
  const mail = mailAccounts(accounts);
  const senders = senderChoices(mail);

  const update = (patch: Partial<Fields>) => {
    setFields((f) => ({ ...f, ...patch }));
    setVersion((v) => v + 1);
    // A problem under a field goes away as soon as that field is changed.
    setProblem((p) => (p && p.field in patch ? null : p));
  };

  const buildDraft = useCallback(
    (id: string | null): Draft => ({
      ...initial,
      id,
      accountId: fields.accountId,
      fromAddress: fields.fromAddress,
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
      draftId.current = await api.saveDraft(buildDraft(draftId.current));
      setIsSavedNoteShown(true);
      setIsCloseRefused(false);
      return true;
    } catch (e: unknown) {
      const err = toVemError(e);
      lastSaveError.current = err;
      setError(err);
      return false;
    }
  }, [buildDraft]);

  // Left through the menu and the last save failed: the app says so, since this screen is gone.
  const onLastSaveFailed = useCallback(() => {
    if (lastSaveError.current) showError(lastSaveError.current);
  }, [showError]);

  const autosave = useAutosave({ version, hasContent, intervalMs: autosaveMs, save, onLastSaveFailed });

  const handleClose = async () => {
    if (isClosing.current) return;
    isClosing.current = true;
    // Close only when the text is safe in Concepten; otherwise the error stays, with the text.
    if (await autosave.flush()) {
      onClose();
      return;
    }
    isClosing.current = false;
    setIsCloseRefused(true);
  };
  // Android's back key is Sluiten: the text is kept in Concepten first, never thrown away.
  useBackStep(true, () => {
    if (!isSending) void handleClose();
  }, 1);

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
    if (isSendingRef.current) return;
    isSendingRef.current = true;
    setIsSending(true);
    const release = () => {
      isSendingRef.current = false;
      setIsSending(false);
    };
    // Let a running save finish first and stop saving after it, or a copy stays behind in Concepten.
    if (!(await autosave.flush())) {
      release();
      return;
    }
    autosave.stop();
    try {
      const item = await api.sendMessage(buildDraft(draftId.current));
      startSend(item);
      go({ kind: "mail" });
    } catch (e: unknown) {
      autosave.resume();
      setError(toVemError(e));
      release();
    }
  };

  const handleSend = () => {
    if (isSendingRef.current) return;
    const found = addressProblem(fields);
    setProblem(found);
    if (found) {
      (found.field === "cc" ? ccRef : toRef).current?.focus();
      return;
    }
    const count = parseAddressList(fields.to).addresses.length + parseAddressList(fields.cc).addresses.length;
    if (count > MANY_RECIPIENTS) return setIsConfirming(true);
    void send();
  };

  const recipientCount = parseAddressList(fields.to).addresses.length + parseAddressList(fields.cc).addresses.length;
  const problemFor = (field: AddressFieldName) => (problem?.field === field ? problem.text : null);
  const title = initial.inReplyTo ? t("compose.titleReply") : t("compose.title");

  return (
    <section className="compose" aria-label={title}>
      <header className="compose-header">
        <h2>{title}</h2>
        <p className="save-note" role="status">
          {isSavedNoteShown ? t("compose.saved") : ""}
        </p>
        <div className="button-row">
          <button type="button" className="primary" disabled={isSending} onClick={handleSend}>
            {t("compose.send")}
          </button>
          <button type="button" disabled={isSending} onClick={handleClose}>{t("compose.close")}</button>
        </div>
      </header>
      {error && <ErrorBox error={error} onClose={() => setError(null)} />}
      {isCloseRefused && <p className="form-problem" role="alert">{t("compose.notSavedStays")}</p>}
      {senders.length > 1 && (
        <div className="form-row">
          <label htmlFor="compose-from">{t("compose.from")}</label>
          <select
            id="compose-from"
            value={senderValue(fields.accountId, fields.fromAddress)}
            onChange={(e) => {
              const picked = senders.find((s) => s.value === e.target.value);
              if (picked) update({ accountId: picked.accountId, fromAddress: picked.fromAddress });
            }}
          >
            {senders.map((s) => (
              <option key={s.value} value={s.value}>{s.label}</option>
            ))}
          </select>
        </div>
      )}
      <AddressField
        id="compose-to"
        label={t("compose.to")}
        value={fields.to}
        problem={problemFor("to")}
        inputRef={toRef}
        onChange={(to) => update({ to })}
      />
      {isCcShown ? (
        <AddressField
          id="compose-cc"
          label={t("compose.cc")}
          value={fields.cc}
          problem={problemFor("cc")}
          inputRef={ccRef}
          onChange={(cc) => update({ cc })}
        />
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
                <button
                  type="button"
                  aria-label={t("compose.removeAttachmentNamed", { name: fileName(path) })}
                  onClick={() => update({ attachments: fields.attachments.filter((_, j) => j !== i) })}
                >
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
