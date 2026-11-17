import { t } from "../../i18n";

/**
 * Content-Security-Policy for the mail frame. Nothing loads except pictures inside the message,
 * and pictures from the internet only after "Afbeeldingen tonen".
 */
export function mailCsp(allowRemoteImages: boolean): string {
  const img = allowRemoteImages ? "img-src data: cid: https:" : "img-src data: cid:";
  return `default-src 'none'; ${img}; style-src 'unsafe-inline'`;
}

function escapeAttr(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}

/** The whole document for the frame: the CSP comes first, before any of the mail. */
export function buildSrcdoc(html: string, allowRemoteImages: boolean, textSize: number): string {
  const csp = escapeAttr(mailCsp(allowRemoteImages));
  const style =
    `body{font-family:"Segoe UI",system-ui,sans-serif;font-size:${textSize}px;line-height:1.5;` +
    "color:#111;background:#fff;margin:12px;overflow-wrap:anywhere}img{max-width:100%;height:auto}" +
    "a[href]{color:#0a3a8c}";
  return (
    `<!doctype html><html><head><meta charset="utf-8">` +
    `<meta http-equiv="Content-Security-Policy" content="${csp}">` +
    `<style>${style}</style></head><body>${html}</body></html>`
  );
}

/** The only sandbox permission: links may ask for a new window, which the shell catches. */
export const MAIL_SANDBOX = "allow-popups";

/**
 * On a phone. Android's WebView opens no new windows, so a link aimed at one does nothing there.
 * Instead a tapped link may lead the app's own window away, which the shell's navigation handler
 * refuses while it opens the link outside (the same handler as on Windows). Only a tap counts:
 * the frame still runs no scripts, so it cannot navigate by itself.
 */
export const PHONE_MAIL_SANDBOX = "allow-popups allow-top-navigation-by-user-activation";

/** The core gives every link target="_blank"; on a phone it becomes target="_top". */
export function linksToTop(html: string): string {
  return html.replace(/target="_blank"/g, 'target="_top"');
}

interface MailFrameProps {
  html: string;
  allowRemoteImages: boolean;
  textSize: number;
  /** The Android app (AppInfo.isPhone), where links go through the app's own window. */
  isPhoneApp?: boolean;
}

/**
 * HTML mail, only ever inside a sandboxed frame. The sandbox allows popups and nothing else: no scripts,
 * no same origin (so the mail cannot reach the app or the Rust core), and popups stay sandboxed too.
 * The core gives every link target="_blank", so a click becomes a new-window request; the shell denies it
 * and opens http(s) links in the system browser, or our own Write screen for mailto.
 */
export function MailFrame({ html, allowRemoteImages, textSize, isPhoneApp = false }: MailFrameProps) {
  return (
    <iframe
      className="mail-frame"
      title={t("read.frameTitle")}
      sandbox={isPhoneApp ? PHONE_MAIL_SANDBOX : MAIL_SANDBOX}
      referrerPolicy="no-referrer"
      srcDoc={buildSrcdoc(isPhoneApp ? linksToTop(html) : html, allowRemoteImages, textSize)}
    />
  );
}
