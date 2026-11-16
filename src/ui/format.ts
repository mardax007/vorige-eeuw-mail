import { locale, t } from "../i18n";
import type { Addr } from "../api/types";

const dayTime = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" });
const dayMonth = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" });
const fullDate = new Intl.DateTimeFormat(locale, { day: "numeric", month: "numeric", year: "numeric" });
const longDate = new Intl.DateTimeFormat(locale, {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

/** Short date for a list row: the time today, day and month this year, else the full date. */
export function shortDate(iso: string, now: Date = new Date()): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  if (d.toDateString() === now.toDateString()) return dayTime.format(d);
  if (d.getFullYear() === now.getFullYear()) return dayMonth.format(d);
  return fullDate.format(d);
}

export function readableDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : longDate.format(d);
}

export function dateOnly(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : fullDate.format(d);
}

const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return t("size.bytes", { n: bytes });
  if (bytes < 1024 ** 2) return t("size.kb", { n: number.format(bytes / 1024) });
  if (bytes < 1024 ** 3) return t("size.mb", { n: number.format(bytes / 1024 ** 2) });
  return t("size.gb", { n: number.format(bytes / 1024 ** 3) });
}

export function formatKb(kb: number): string {
  return formatBytes(kb * 1024);
}

/** "Name <address>", or just the address. */
export function formatAddr(a: Addr): string {
  return a.name ? `${a.name} <${a.email}>` : a.email;
}

export function displayName(a: Addr): string {
  return a.name && a.name.trim() !== "" ? a.name : a.email;
}

export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}
