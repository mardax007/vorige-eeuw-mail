// Builds the draft for Beantwoorden, Allen beantwoorden and Doorsturen.

import type { Addr, Draft, MessageDetail } from "../../api/types";
import { t } from "../../i18n";
import { displayName, formatAddr, readableDate } from "../format";
import { formatAddressList } from "./address";

export type ReplyKind = "reply" | "replyAll" | "forward";

const REPLY_PREFIX = /^\s*(re|antw|aw)\s*:/i;
const FORWARD_PREFIX = /^\s*(fw|fwd|doorst)\s*:/i;

export function withPrefix(subject: string, kind: ReplyKind): string {
  if (kind === "forward") {
    return FORWARD_PREFIX.test(subject) ? subject : `${t("compose.forwardPrefix")}${subject}`;
  }
  return REPLY_PREFIX.test(subject) ? subject : `${t("compose.replyPrefix")}${subject}`;
}

/** Plain text of a message, also when it only came as HTML. */
export function plainBody(message: Pick<MessageDetail, "text" | "html">): string {
  if (message.text && message.text.trim() !== "") return message.text;
  if (!message.html) return "";
  // DOMParser builds an inert document: no scripts run, nothing loads.
  const doc = new DOMParser().parseFromString(message.html, "text/html");
  return (doc.body.textContent ?? "").trim();
}

function quote(text: string): string {
  return text
    .split(/\r?\n/)
    .map((line) => `> ${line}`)
    .join("\n");
}

function sameAddr(a: Addr, b: Addr): boolean {
  return a.email.toLowerCase() === b.email.toLowerCase();
}

function uniqueWithout(list: readonly Addr[], without: readonly Addr[]): Addr[] {
  return list.reduce<Addr[]>(
    (acc, a) => (acc.some((x) => sameAddr(x, a)) || without.some((x) => sameAddr(x, a)) ? acc : [...acc, a]),
    [],
  );
}

function signatureBlock(signature: string | null | undefined): string {
  return signature && signature.trim() !== "" ? `\n\n${signature}` : "";
}

export function replyRecipients(message: MessageDetail, kind: ReplyKind, ownEmail: string): { to: Addr[]; cc: Addr[] } {
  const own: Addr[] = [{ email: ownEmail }];
  const first = message.replyTo.length > 0 ? message.replyTo : [message.from];
  if (kind === "forward") return { to: [], cc: [] };
  if (kind === "reply") return { to: first, cc: [] };
  const to = uniqueWithout([...first, ...message.to], own);
  const cc = uniqueWithout(message.cc, [...own, ...to]);
  return { to, cc };
}

export function prefillDraft(
  base: Draft,
  message: MessageDetail,
  kind: ReplyKind,
  ownEmail: string,
  signature: string | null | undefined,
): Draft {
  const { to, cc } = replyRecipients(message, kind, ownEmail);
  const body = plainBody(message);
  const sig = signatureBlock(signature);
  const quoted =
    kind === "forward"
      ? [
          t("compose.forwardHeader"),
          `${t("read.from")}: ${formatAddr(message.from)}`,
          `${t("read.date")}: ${readableDate(message.date)}`,
          `${t("read.subject")}: ${message.subject}`,
          `${t("read.to")}: ${formatAddressList(message.to)}`,
          "",
          body,
        ].join("\n")
      : `${t("compose.quoteHeader", { date: readableDate(message.date), name: displayName(message.from) })}\n${quote(body)}`;
  const references = message.messageIdHeader
    ? [...message.references, message.messageIdHeader]
    : message.references;
  return {
    ...base,
    to,
    cc,
    subject: withPrefix(message.subject, kind),
    bodyText: `\n${sig}\n\n${quoted}`,
    inReplyTo: kind === "forward" ? null : (message.messageIdHeader ?? null),
    references: kind === "forward" ? [] : references,
  };
}
