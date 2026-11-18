import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { toVemError } from "../../api/errors";
import type { MailWarning, MessageDetail, VemError } from "../../api/types";
import { t } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { TagChip } from "../common/TagChip";
import { formatAddressList } from "../compose/address";
import { displayName, formatBytes, readableDate } from "../format";
import { useApp } from "../state/AppContext";
import { LabelMenu } from "./LabelMenu";
import { MailFrame } from "./MailFrame";
import { MoveMenu } from "./MoveMenu";
import { useMessageActions } from "./useMessageActions";

interface ReadingPaneProps {
  messageId: string;
  /** Shown as a big "Terug" button when the message lies over the list. */
  onBack?: () => void;
  onRemoved: () => void;
}

function warningText(w: MailWarning): string {
  const vars = { shown: w.shown ?? "", actual: w.actual ?? "" };
  if (w.kind === "linkMismatch") return t("warn.linkMismatch", vars);
  if (w.kind === "lookalikeContact") return t("warn.lookalikeContact", vars);
  return t("warn.firstContactMoney");
}

type Panel = "none" | "label" | "move";

/** The open message: one row of labelled buttons, who sent it, and the body. */
export function ReadingPane({ messageId, onBack, onRemoved }: ReadingPaneProps) {
  const { tags, folders, settings, showError, compose, reload, appInfo } = useApp();
  const { remove, move, respond, isForwarding } = useMessageActions();
  const [message, setMessage] = useState<MessageDetail | null>(null);
  const [loadError, setLoadError] = useState<VemError | null>(null);
  const [showImages, setShowImages] = useState(false);
  const [panel, setPanel] = useState<Panel>("none");
  const [savedName, setSavedName] = useState<string | null>(null);
  // Goes up on every load: an answer for an older load (another message, by now) is ignored.
  const request = useRef(0);

  const load = useCallback(
    async (remote: boolean) => {
      const token = ++request.current;
      let m: MessageDetail;
      try {
        m = await api.getMessage(messageId, remote);
      } catch (e: unknown) {
        if (token === request.current) setLoadError(toVemError(e));
        return;
      }
      if (token !== request.current) return;
      setMessage(m);
      if (!m.unread) return;
      // The message is open either way; a failure here only means it stays bold in the list.
      try {
        await api.setRead([m.id], true);
        await reload();
      } catch (e: unknown) {
        showError(e);
      }
    },
    [messageId, reload, showError],
  );

  useEffect(() => {
    setMessage(null);
    setLoadError(null);
    setShowImages(false);
    setPanel("none");
    setSavedName(null);
    void load(false);
    // Only a different message loads anew; showing images loads on its own below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messageId]);

  if (!message) {
    return (
      <section className="reading-pane">
        {onBack && <BackButton onBack={onBack} />}
        {loadError ? (
          <>
            <p className="list-note">{t("read.loadFailed")}</p>
            <ErrorBox error={loadError} />
            {!onBack && <BackButton onBack={onRemoved} />}
          </>
        ) : (
          <p className="list-note" role="status">{t("read.loading")}</p>
        )}
      </section>
    );
  }

  const folder = folders.find((f) => f.id === message.folderId);
  const accountFolders = folders.filter((f) => f.accountId === message.accountId);
  const messageTags = tags.filter((tag) => message.tagIds.includes(tag.id));
  const isDraft = folder?.role === "drafts";
  // In Verwijderde items Verwijderen would move nothing; emptying the folder is its own button.
  const canDelete = folder?.role !== "trash";
  // "Ongewenste e-mail" moves to the account's junk folder; inside it the button moves back to
  // Postvak IN. Without a junk folder (mail kept on the PC) there is no button (decision 29).
  const isJunk = folder?.role === "junk";
  const junkTarget = accountFolders.find((f) => f.role === (isJunk ? "inbox" : "junk"));

  const toggleImportant = async () => {
    try {
      await api.setImportant([message.id], !message.important);
      const important = !message.important;
      setMessage((m) => (m && m.id === message.id ? { ...m, important } : m));
      await reload();
    } catch (e: unknown) {
      showError(e);
    }
  };

  const openDraft = async () => {
    try {
      compose(await api.loadDraft(message.id));
    } catch (e: unknown) {
      showError(e);
    }
  };

  const handleShowImages = async () => {
    setShowImages(true);
    const token = ++request.current;
    try {
      const m = await api.getMessage(message.id, true);
      if (token === request.current) setMessage(m);
    } catch (e: unknown) {
      if (token === request.current) showError(e);
    }
  };

  const handleAttachment = async (attachmentId: string, name: string, isSave: boolean) => {
    setSavedName(null);
    try {
      if (!isSave) {
        await api.openAttachment(message.id, attachmentId);
        return;
      }
      if (await api.saveAttachment(message.id, attachmentId)) setSavedName(name);
    } catch (e: unknown) {
      showError(e);
    }
  };

  // On a phone the screen is narrow: who wrote it comes first, the buttons below it.
  const isPhone = appInfo?.isPhone === true;
  const header = <MessageHeader message={message} />;

  return (
    <section className="reading-pane" aria-label={message.subject}>
      {onBack && <BackButton onBack={onBack} />}
      {isPhone && header}
      <div className="button-row action-row no-print">
        {isDraft && (
          <button type="button" className="primary" onClick={openDraft}>
            {t("read.editDraft")}
          </button>
        )}
        <button type="button" onClick={() => respond(message, "reply")}>{t("read.reply")}</button>
        <button type="button" onClick={() => respond(message, "replyAll")}>{t("read.replyAll")}</button>
        <button type="button" disabled={isForwarding} onClick={() => respond(message, "forward")}>{t("read.forward")}</button>
        {canDelete && (
          <button type="button" onClick={() => remove([message.id]).then((ok) => ok && onRemoved())}>{t("read.delete")}</button>
        )}
        {junkTarget && (
          <button type="button" onClick={() => void move([message.id], junkTarget.id).then((ok) => ok && onRemoved())}>
            {isJunk ? t("read.notJunk") : t("read.junk")}
          </button>
        )}
        <button type="button" aria-expanded={panel === "move"} onClick={() => setPanel(panel === "move" ? "none" : "move")}>
          {t("read.move")}
        </button>
        <button type="button" aria-expanded={panel === "label"} onClick={() => setPanel(panel === "label" ? "none" : "label")}>
          {t("read.label")}
        </button>
        <button type="button" aria-pressed={message.important} onClick={toggleImportant}>
          {message.important ? `★ ${t("read.unmarkImportant")}` : `☆ ${t("read.markImportant")}`}
        </button>
        {/* Android's WebView has no print dialog behind window.print(). */}
        {!isPhone && (
          <button type="button" onClick={() => window.print()}>{t("read.print")}</button>
        )}
      </div>
      {isForwarding && (
        <p className="calm-line no-print" role="status">
          {t("read.forwardPreparing")}
        </p>
      )}
      {panel === "move" && (
        <MoveMenu
          folders={accountFolders}
          currentFolderId={message.folderId}
          onClose={() => setPanel("none")}
          onMove={(folderId) => {
            setPanel("none");
            void move([message.id], folderId).then((ok) => ok && onRemoved());
          }}
        />
      )}
      {panel === "label" && (
        <LabelMenu
          messageId={message.id}
          tags={tags}
          activeTagIds={message.tagIds}
          onClose={() => setPanel("none")}
          onError={showError}
          onChanged={() => void reload().then(() => load(showImages))}
        />
      )}
      {!isPhone && header}
      {messageTags.length > 0 && (
        <p className="message-tags">
          {messageTags.map((tag) => (
            <TagChip key={tag.id} tag={tag} />
          ))}
        </p>
      )}
      {message.warnings.map((w) => (
        <p key={`${w.kind}-${w.actual ?? ""}`} className="mail-warning" role="note">
          {warningText(w)}
        </p>
      ))}
      {message.html && message.hasRemoteImages && !message.remoteImagesShown && (
        <div className="images-blocked no-print">
          <span>{t("read.imagesBlocked")}</span>
          <button type="button" onClick={handleShowImages}>{t("read.showImages")}</button>
        </div>
      )}
      {message.attachments.length > 0 && (
        <div className="attachments">
          <h3>{t("read.attachments")}</h3>
          <ul>
            {message.attachments.map((a) => (
              <li key={a.id} className="attachment-row">
                <span>{a.name} ({formatBytes(a.size)})</span>
                <button type="button" onClick={() => handleAttachment(a.id, a.name, false)}>{t("read.open")}</button>
                <button type="button" onClick={() => handleAttachment(a.id, a.name, true)}>{t("read.save")}</button>
              </li>
            ))}
          </ul>
          <p className="save-note" role="status">
            {savedName ? t("read.saved", { name: savedName }) : ""}
          </p>
        </div>
      )}
      <div className="mail-body">
        {message.html ? (
          <MailFrame
            html={message.html}
            allowRemoteImages={showImages || message.remoteImagesShown}
            textSize={settings.textSize}
            isPhoneApp={appInfo?.isPhone === true}
          />
        ) : (
          <pre className="mail-text">{message.text ?? ""}</pre>
        )}
      </div>
    </section>
  );
}

function BackButton({ onBack }: { onBack: () => void }) {
  return (
    <button type="button" className="back-button no-print" onClick={onBack}>
      ← {t("nav.back")}
    </button>
  );
}

function MessageHeader({ message }: { message: MessageDetail }) {
  return (
    <header className="message-header">
      <h2>{message.subject || t("list.noSubject")}</h2>
      <dl>
        <dt>{t("read.from")}</dt>
        <dd>
          <strong>{displayName(message.from)}</strong> <span className="real-address">&lt;{message.from.email}&gt;</span>
        </dd>
        <dt>{t("read.to")}</dt>
        <dd>{formatAddressList(message.to)}</dd>
        {message.cc.length > 0 && (
          <>
            <dt>{t("read.cc")}</dt>
            <dd>{formatAddressList(message.cc)}</dd>
          </>
        )}
        <dt>{t("read.date")}</dt>
        <dd>{readableDate(message.date)}</dd>
      </dl>
    </header>
  );
}
