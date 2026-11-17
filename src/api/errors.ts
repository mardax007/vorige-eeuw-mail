import type { VemError } from "./types";

/** Code used when something fails that the core did not describe. */
export const UNKNOWN_ERROR_CODE = "VEM-APP-CMD-01";

export function isVemError(value: unknown): value is VemError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    typeof (value as { code: unknown }).code === "string"
  );
}

/** True for the error a browser sign-in ends with after cancel_sign_in ("Stoppen"). */
export function isStoppedSignIn(error: VemError): boolean {
  return error.code === "VEM-AUTH-LOGIN-01" && error.detail === "stopped";
}

/** Turns anything thrown into a VemError, so every screen can show it the same way. */
export function toVemError(value: unknown): VemError {
  if (isVemError(value)) {
    return {
      code: value.code,
      serverReply: value.serverReply ?? null,
      detail: value.detail ?? null,
    };
  }
  return { code: UNKNOWN_ERROR_CODE, serverReply: null, detail: errorText(value) };
}

/**
 * The readable text of something thrown. Tauri's mobile plugins reject with a plain
 * `{ message }` object, which `String()` would turn into "[object Object]".
 */
export function errorText(value: unknown): string {
  if (value instanceof Error) return value.message;
  if (typeof value === "object" && value !== null && "message" in value) {
    const message = (value as { message: unknown }).message;
    if (typeof message === "string") return message;
  }
  return String(value);
}
