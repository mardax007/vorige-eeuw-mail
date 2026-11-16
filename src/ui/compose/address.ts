import type { Addr } from "../../api/types";
import { formatAddr } from "../format";

const EMAIL = /^[^\s@<>,;]+@[^\s@<>,;]+\.[^\s@<>,;]+$/;

export function isEmail(value: string): boolean {
  return EMAIL.test(value.trim());
}

/** Splits on commas and semicolons that are not inside quotes. */
function splitList(text: string): string[] {
  const parts: string[] = [];
  let current = "";
  let quoted = false;
  for (const ch of text) {
    if (ch === '"') quoted = !quoted;
    if ((ch === "," || ch === ";") && !quoted) {
      parts.push(current);
      current = "";
    } else {
      current += ch;
    }
  }
  parts.push(current);
  return parts.map((p) => p.trim()).filter((p) => p !== "");
}

export interface ParsedList {
  addresses: Addr[];
  invalid: string[];
}

/** Reads "Jan <jan@voorbeeld.nl>, piet@voorbeeld.nl" into addresses. */
export function parseAddressList(text: string): ParsedList {
  return splitList(text).reduce<ParsedList>(
    (acc, part) => {
      const angle = part.match(/^(.*)<([^>]+)>$/);
      const name = angle ? angle[1].trim().replace(/^"|"$/g, "") : "";
      const email = (angle ? angle[2] : part).trim();
      return isEmail(email)
        ? { ...acc, addresses: [...acc.addresses, { name: name || null, email }] }
        : { ...acc, invalid: [...acc.invalid, part] };
    },
    { addresses: [], invalid: [] },
  );
}

export function formatAddressList(list: readonly Addr[]): string {
  return list.map(formatAddr).join(", ");
}

/** The part after the last comma, which the user is typing now. */
export function lastToken(text: string): string {
  const parts = text.split(/[,;]/);
  return parts[parts.length - 1].trim();
}

/** Puts a chosen address in place of the part being typed. */
export function replaceLastToken(text: string, addr: Addr): string {
  const cut = Math.max(text.lastIndexOf(","), text.lastIndexOf(";"));
  const head = cut >= 0 ? `${text.slice(0, cut + 1)} ` : "";
  return `${head}${formatAddr(addr)}, `;
}
