import type { Attachment, Invite } from "../../api/types";
import { t } from "../../i18n";
import { displayName } from "../format";
import { inviteWhen } from "./invite";

interface InviteBoxProps {
  invite: Invite;
  attachments: Attachment[];
}

/** The calendar file to point at, when the invitation came with one. */
function calendarFile(attachments: Attachment[]): string | null {
  const ics = attachments.find((a) => a.mime === "text/calendar" || a.name.toLowerCase().endsWith(".ics"));
  return ics?.name ?? null;
}

/**
 * An invitation, readably: what, when, where and who invites (C11, decision 29). There is no
 * calendar and nothing is answered from here; the attached calendar file opens in one.
 */
export function InviteBox({ invite, attachments }: InviteBoxProps) {
  const when = invite.start ? inviteWhen(invite.start, invite.end) : "";
  const file = calendarFile(attachments);
  const title = invite.cancelled ? t("invite.cancelledTitle") : t("invite.title");
  return (
    <section className={invite.cancelled ? "invite-box cancelled" : "invite-box"} aria-label={title}>
      <h3>{title}</h3>
      {invite.cancelled && <p className="invite-cancelled">{t("invite.cancelled")}</p>}
      <dl>
        {invite.summary && (
          <>
            <dt>{t("invite.what")}</dt>
            <dd>{invite.summary}</dd>
          </>
        )}
        {when && (
          <>
            <dt>{t("invite.when")}</dt>
            <dd>{when}</dd>
          </>
        )}
        {invite.location && (
          <>
            <dt>{t("invite.where")}</dt>
            <dd>{invite.location}</dd>
          </>
        )}
        {invite.organizer && (
          <>
            <dt>{t("invite.from")}</dt>
            <dd>{displayName(invite.organizer)}</dd>
          </>
        )}
      </dl>
      {invite.repeats && <p>{t("invite.repeats")}</p>}
      {file && !invite.cancelled && <p className="invite-note">{t("invite.noCalendar", { name: file })}</p>}
    </section>
  );
}
