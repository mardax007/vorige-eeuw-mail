import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { buildSrcdoc, MailFrame, mailCsp } from "./MailFrame";

describe("MailFrame", () => {
  it("renders an iframe whose sandbox allows only popups, with a CSP in the srcdoc", () => {
    render(<MailFrame html="<p>Hallo <script>alert(1)</script></p>" allowRemoteImages={false} textSize={18} />);
    const frame = document.querySelector("iframe");
    expect(frame).not.toBeNull();
    const sandbox = frame?.getAttribute("sandbox") ?? "missing";
    expect(sandbox).toBe("allow-popups");
    expect(sandbox).not.toContain("allow-scripts");
    expect(sandbox).not.toContain("allow-same-origin");
    expect(sandbox).not.toContain("allow-popups-to-escape-sandbox");
    expect(sandbox).not.toContain("allow-top-navigation");
    const srcdoc = frame?.getAttribute("srcdoc") ?? "";
    expect(srcdoc).toContain('http-equiv="Content-Security-Policy"');
    expect(srcdoc).toContain("default-src 'none'");
    expect(srcdoc).not.toContain("https:");
  });

  it("puts the CSP before the mail content", () => {
    const doc = buildSrcdoc("<meta http-equiv=\"refresh\" content=\"0\"><p>x</p>", false, 18);
    expect(doc.indexOf("Content-Security-Policy")).toBeLessThan(doc.indexOf("<p>x</p>"));
    expect(doc.indexOf("Content-Security-Policy")).toBeLessThan(doc.indexOf("refresh"));
  });

  it("allows https images only after Afbeeldingen tonen", () => {
    expect(mailCsp(false)).toBe("default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'");
    expect(mailCsp(true)).toBe("default-src 'none'; img-src data: cid: https:; style-src 'unsafe-inline'");
  });
});
