import type { SignIn, VemError } from "../../api/types";

const GOOGLE_DOMAINS: readonly string[] = ["gmail.com", "googlemail.com"];
const MICROSOFT_DOMAINS: readonly string[] = [
  "outlook.com",
  "outlook.nl",
  "hotmail.com",
  "hotmail.nl",
  "live.com",
  "live.nl",
  "msn.com",
];

/** Code the core uses when a Google or Microsoft sign-in has run out. */
export const EXPIRED_SIGN_IN_CODE = "VEM-AUTH-LOGIN-02";

/** How a mailbox signs in again: a new password, or the browser (Google, Microsoft). */
export type SignInRoute = "password" | "browser";

/**
 * Which provider an address belongs to. The core does not send how an added mailbox signs in,
 * so the screens read it from the address, as the provider table does for the well-known ones.
 */
export function providerOf(email: string): SignIn {
  const domain = email.split("@")[1]?.trim().toLowerCase() ?? "";
  if (GOOGLE_DOMAINS.includes(domain)) return "google";
  if (MICROSOFT_DOMAINS.includes(domain)) return "microsoft";
  return "password";
}

/**
 * The way to offer: Microsoft always signs in through the browser (decision 9), Google only when
 * that sign-in is available (otherwise it uses an app password), and an expired sign-in always does.
 */
export function signInRoute(provider: SignIn, isOAuthAvailable: boolean, error?: VemError | null): SignInRoute {
  if (error?.code === EXPIRED_SIGN_IN_CODE) return "browser";
  if (provider === "microsoft") return "browser";
  if (provider === "google" && isOAuthAvailable) return "browser";
  return "password";
}
