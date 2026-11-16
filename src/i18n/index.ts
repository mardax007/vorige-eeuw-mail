import { UNKNOWN_ERROR_CODE } from "../api/errors";
import { locale, nl, type MessageKey } from "./nl";
import { nlErrors, type ErrorText } from "./nlErrors";

export type { MessageKey } from "./nl";
export { ERROR_CODES } from "./nlErrors";
export { locale };

export type Vars = Readonly<Record<string, string | number>>;

/** Fills {name} placeholders. A placeholder without a value stays as it is. */
export function interpolate(text: string, vars?: Vars): string {
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : whole,
  );
}

/** The screen text for a key. */
export function t(key: MessageKey, vars?: Vars): string {
  return interpolate(nl[key], vars);
}

/** Sentence and next step for an error code; unknown codes get the general text. */
export function errorText(code: string): ErrorText {
  if (Object.prototype.hasOwnProperty.call(nlErrors, code)) {
    return nlErrors[code as keyof typeof nlErrors];
  }
  return nlErrors[UNKNOWN_ERROR_CODE as keyof typeof nlErrors];
}
