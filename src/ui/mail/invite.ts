import { locale, t } from "../../i18n";
import type { InviteTime } from "../../api/types";

const MINUTE_MS = 60_000;
const DAY_MS = 86_400_000;

/** [year, month (1-12), day, hour, minute, second] from "2026-11-20T14:00:00" or "2026-11-20". */
function wallParts(wall: string): number[] | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}):(\d{2}))?$/.exec(wall);
  if (!m) return null;
  return m.slice(1).map((v) => Number(v ?? 0));
}

/** How far `zone` is ahead of UTC at `utcMs`, in milliseconds. Throws for an unknown zone. */
function zoneOffset(zone: string, utcMs: number): number {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: zone,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).formatToParts(new Date(utcMs));
  const get = (type: string) => Number(parts.find((p) => p.type === type)?.value ?? 0);
  const asUtc = Date.UTC(get("year"), get("month") - 1, get("day"), get("hour"), get("minute"), get("second"));
  return asUtc - Math.floor(utcMs / 1000) * 1000;
}

/** The moment a clock time in `zone` stands for, or null when the zone is unknown here. */
export function zonedToDate(wall: string, zone: string): Date | null {
  const p = wallParts(wall);
  if (!p) return null;
  const guess = Date.UTC(p[0], p[1] - 1, p[2], p[3], p[4], p[5]);
  try {
    // Twice, so a time just after a clock change gets the offset that applies then.
    const first = guess - zoneOffset(zone, guess);
    return new Date(guess - zoneOffset(zone, first));
  } catch {
    return null;
  }
}

interface Moment {
  date: Date;
  /** The invitation's own zone name, when this device does not know it: shown next to the time. */
  unknownZone: string | null;
}

function moment(time: InviteTime): Moment | null {
  const p = wallParts(time.wall);
  if (!p) return null;
  const local = new Date(p[0], p[1] - 1, p[2], p[3], p[4], p[5]);
  if (!time.zone) return { date: local, unknownZone: null };
  if (time.zone === "UTC") return { date: new Date(Date.UTC(p[0], p[1] - 1, p[2], p[3], p[4], p[5])), unknownZone: null };
  const zoned = zonedToDate(time.wall, time.zone);
  return zoned ? { date: zoned, unknownZone: null } : { date: local, unknownZone: time.zone };
}

/**
 * "vrijdag 20 november 2026, 14:00 tot 17:00", in this device's time. `timeZone` is for tests.
 * A whole-day invitation ends the day before its end date (iCalendar's end is exclusive).
 */
export function inviteWhen(start: InviteTime, end: InviteTime | null | undefined, timeZone?: string): string {
  const dayFormat = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long", year: "numeric", timeZone });
  const timeFormat = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", timeZone });
  if (start.allDay) {
    const s = wallParts(start.wall);
    if (!s) return "";
    const utcDay = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });
    const first = Date.UTC(s[0], s[1] - 1, s[2]);
    const e = end?.allDay ? wallParts(end.wall) : null;
    const last = e ? Date.UTC(e[0], e[1] - 1, e[2]) - DAY_MS : first;
    if (last > first) return t("invite.days", { from: utcDay.format(first), to: utcDay.format(last) });
    return t("invite.allDay", { date: utcDay.format(first) });
  }
  const from = moment(start);
  if (!from) return "";
  const withZone = (m: Moment, text: string) => (m.unknownZone ? t("invite.zone", { time: text, zone: m.unknownZone }) : text);
  const to = end && !end.allDay ? moment(end) : null;
  const startDay = dayFormat.format(from.date);
  const startTime = timeFormat.format(from.date);
  if (!to || to.date.getTime() - from.date.getTime() < MINUTE_MS) return withZone(from, `${startDay}, ${startTime}`);
  const endDay = dayFormat.format(to.date);
  const endTime = timeFormat.format(to.date);
  if (endDay === startDay) return withZone(from, t("invite.sameDay", { date: startDay, start: startTime, end: endTime }));
  return withZone(from, t("invite.range", { start: `${startDay}, ${startTime}`, end: `${endDay}, ${endTime}` }));
}
