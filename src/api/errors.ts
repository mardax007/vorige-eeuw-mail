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

/** Turns anything thrown into a VemError, so every screen can show it the same way. */
export function toVemError(value: unknown): VemError {
  if (isVemError(value)) {
    return {
      code: value.code,
      serverReply: value.serverReply ?? null,
      detail: value.detail ?? null,
    };
  }
  const detail = value instanceof Error ? value.message : String(value);
  return { code: UNKNOWN_ERROR_CODE, serverReply: null, detail };
}
