import { describe, expect, it } from "vitest";
import { UNKNOWN_ERROR_CODE } from "../api/errors";
import { ERROR_CODES, errorText, interpolate, t } from "./index";
import { nl } from "./nl";
import { nlErrors } from "./nlErrors";

/** The codes the Rust core emits (coordinator's list, docs/errors.md scheme). */
const CORE_CODES = [
  "VEM-LOOKUP-FIND-01",
  "VEM-IMAP-FIND-01", "VEM-IMAP-CONNECT-01", "VEM-IMAP-CONNECT-02", "VEM-IMAP-TLS-01", "VEM-IMAP-TLS-02",
  "VEM-IMAP-LOGIN-01", "VEM-IMAP-LOGIN-02", "VEM-IMAP-CMD-01", "VEM-IMAP-CMD-02",
  "VEM-SMTP-FIND-01", "VEM-SMTP-CONNECT-01", "VEM-SMTP-TLS-01", "VEM-SMTP-TLS-02", "VEM-SMTP-LOGIN-01",
  "VEM-SMTP-CMD-01", "VEM-SMTP-CMD-02", "VEM-SMTP-CMD-03",
  "VEM-AUTH-LOGIN-01", "VEM-AUTH-LOGIN-02",
  "VEM-IMPORT-READ-01", "VEM-IMPORT-READ-02", "VEM-IMPORT-READ-03", "VEM-IMPORT-SPACE-01", "VEM-IMPORT-SPACE-02",
  "VEM-STORE-WRITE-01", "VEM-STORE-READ-01", "VEM-STORE-SECRET-01",
  "VEM-UPDATE-CMD-01", "VEM-REPORT-CMD-01", "VEM-APP-CMD-01",
];

describe("error catalogue", () => {
  it("has exactly the codes the core emits", () => {
    expect([...ERROR_CODES].sort()).toEqual([...CORE_CODES].sort());
  });

  it("gives every code a sentence and a next step", () => {
    for (const code of ERROR_CODES) {
      const text = nlErrors[code];
      expect(text.sentence.trim(), code).not.toBe("");
      expect(text.next.trim(), code).not.toBe("");
      expect(text.sentence.endsWith("."), code).toBe(true);
    }
  });

  it("follows the VEM-<AREA>-<STAGE>-<NN> scheme", () => {
    for (const code of ERROR_CODES) expect(code).toMatch(/^VEM-[A-Z]+-[A-Z]+-\d{2}$/);
  });

  it("falls back to the general text for an unknown code", () => {
    expect(errorText("VEM-NEW-THING-99")).toEqual(nlErrors[UNKNOWN_ERROR_CODE as keyof typeof nlErrors]);
  });

  it("uses no technical words in sentences", () => {
    for (const code of ERROR_CODES) {
      const { sentence, next } = nlErrors[code];
      expect(`${sentence} ${next}`, code).not.toMatch(/\b(IMAP|SMTP|SSL|TLS|OAuth|port|cache|sync)\b/i);
    }
  });
});

describe("screen texts", () => {
  it("has no empty strings", () => {
    for (const [key, value] of Object.entries(nl)) expect(value.trim(), key).not.toBe("");
  });

  it("uses Outlook's Dutch words", () => {
    expect(t("folder.inbox")).toBe("Postvak IN");
    expect(t("folder.sent")).toBe("Verzonden items");
    expect(t("folder.trash")).toBe("Verwijderde items");
    expect(t("folder.drafts")).toBe("Concepten");
    expect(t("folder.junk")).toBe("Ongewenste e-mail");
    expect(t("folder.outbox")).toBe("Postvak UIT");
    expect(t("nav.newMail")).toBe("Nieuwe e-mail");
    expect(t("error.copyForMartijn")).toBe("Kopiëren voor Martijn");
  });
});

describe("t and interpolate", () => {
  it("fills placeholders", () => {
    expect(t("compose.sending", { seconds: 7 })).toBe("Wordt verstuurd over 7 seconden.");
    expect(interpolate("{a} en {b}", { a: "Jan", b: 2 })).toBe("Jan en 2");
  });

  it("fills the same placeholder twice", () => {
    expect(interpolate("{x}-{x}", { x: "a" })).toBe("a-a");
  });

  it("leaves a placeholder without value as it is", () => {
    expect(interpolate("Hallo {name}", {})).toBe("Hallo {name}");
    expect(interpolate("Hallo {name}")).toBe("Hallo {name}");
  });

  it("does not read inherited properties", () => {
    expect(interpolate("{toString}", {})).toBe("{toString}");
  });
});
