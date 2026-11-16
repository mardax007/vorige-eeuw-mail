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
    "a{color:#0a3a8c}";
  return (
    `<!doctype html><html><head><meta charset="utf-8">` +
    `<meta http-equiv="Content-Security-Policy" content="${csp}">` +
    `<style>${style}</style></head><body>${html}</body></html>`
  );
}

interface MailFrameProps {
  html: string;
  allowRemoteImages: boolean;
  textSize: number;
}

/**
 * HTML mail, only ever inside a sandboxed frame with an empty sandbox: no scripts, no same origin,
 * so the mail cannot reach the app or the Rust core. Links are caught by the shell's navigation handler.
 */
export function MailFrame({ html, allowRemoteImages, textSize }: MailFrameProps) {
  return (
    <iframe
      className="mail-frame"
      title={t("read.frameTitle")}
      sandbox=""
      referrerPolicy="no-referrer"
      srcDoc={buildSrcdoc(html, allowRemoteImages, textSize)}
    />
  );
}
