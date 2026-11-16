import { api } from "../../api/client";
import type { OutboxItem } from "../../api/types";
import { t } from "../../i18n";
import { ErrorBox } from "../common/ErrorBox";
import { readableDate } from "../format";
import { useApp } from "../state/AppContext";

const STATE_KEYS = {
  waiting: "outbox.waiting",
  sending: "outbox.sending",
  failed: "outbox.failed",
} as const;

/** Postvak UIT: what is still waiting to go, and what could not be sent. */
export function OutboxView() {
  const { outbox, compose, showError } = useApp();

  const takeBack = async (item: OutboxItem) => {
    try {
      compose(await api.cancelSend(item.id));
    } catch (e: unknown) {
      showError(e);
    }
  };

  return (
    <section className="message-list">
      <header className="list-header">
        <h2>{t("folder.outbox")}</h2>
      </header>
      {outbox.length === 0 && <p className="list-note">{t("outbox.empty")}</p>}
      <ul className="outbox-list">
        {outbox.map((item) => (
          <li key={item.id} className="outbox-item">
            <p>
              <strong>{item.subject || t("list.noSubject")}</strong>
            </p>
            <p>
              {t(STATE_KEYS[item.state])} · {readableDate(item.sendAt)}
            </p>
            {item.error && <ErrorBox error={item.error} />}
            {item.state !== "sending" && (
              <button type="button" onClick={() => takeBack(item)}>
                {t("outbox.edit")}
              </button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
