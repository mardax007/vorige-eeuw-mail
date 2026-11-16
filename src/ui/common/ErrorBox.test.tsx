import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ErrorBox, textForMartijn } from "./ErrorBox";

const REPLY = "550 5.7.30 Basic authentication is not supported for Client Submission.\n  second  line\twith tab ";

function stubClipboard() {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  return writeText;
}

afterEach(() => {
  Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
});

describe("ErrorBox", () => {
  it("shows sentence, next step, the exact server reply and the code", () => {
    render(<ErrorBox error={{ code: "VEM-SMTP-LOGIN-01", serverReply: REPLY }} />);
    expect(screen.getByText("Inloggen om mail te versturen is niet gelukt.")).toBeTruthy();
    expect(screen.getByText(/Controleer het wachtwoord/)).toBeTruthy();
    expect(screen.getByText("De server zei:")).toBeTruthy();
    const pre = document.querySelector("pre.error-reply");
    expect(pre?.textContent).toBe(REPLY);
    expect(screen.getByText("Code: VEM-SMTP-LOGIN-01")).toBeTruthy();
  });

  it("labels local error text as Details", () => {
    render(<ErrorBox error={{ code: "VEM-STORE-WRITE-01", detail: "Er is onvoldoende schijfruimte. (os error 112)" }} />);
    expect(screen.getByText("Details:")).toBeTruthy();
    expect(screen.queryByText("De server zei:")).toBeNull();
    expect(document.querySelector("pre")?.textContent).toBe("Er is onvoldoende schijfruimte. (os error 112)");
  });

  it("leaves the box out when there is no text", () => {
    render(<ErrorBox error={{ code: "VEM-IMAP-CMD-02" }} />);
    expect(document.querySelector("pre")).toBeNull();
    expect(screen.queryByText("Kopiëren")).toBeNull();
  });

  it("shows the general text but the real code for an unknown code", () => {
    render(<ErrorBox error={{ code: "VEM-NIEUW-CMD-07" }} />);
    expect(screen.getByText("Er ging iets onverwachts mis.")).toBeTruthy();
    expect(screen.getByText("Code: VEM-NIEUW-CMD-07")).toBeTruthy();
  });

  it("copies the exact reply, and sentence plus reply plus code for Martijn", async () => {
    const writeText = stubClipboard();
    const error = { code: "VEM-IMAP-LOGIN-01", serverReply: REPLY };
    render(<ErrorBox error={error} />);
    fireEvent.click(screen.getByRole("button", { name: "Kopiëren" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(REPLY));
    fireEvent.click(screen.getByRole("button", { name: "Kopiëren voor Martijn" }));
    await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(textForMartijn(error)));
    const copied = textForMartijn(error);
    expect(copied).toContain("Inloggen bij uw postvak is niet gelukt.");
    expect(copied).toContain(REPLY);
    expect(copied).toContain("Code: VEM-IMAP-LOGIN-01");
  });
});
