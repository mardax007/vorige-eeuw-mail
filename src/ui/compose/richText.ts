import { t } from "../../i18n";

/** Tags that mean the text has formatting or photos; without them the message goes as plain text. */
const FORMATTING = "b,strong,i,em,u,ul,ol,img,font,span[style]";
const BLOCKS = new Set(["DIV", "P", "LI", "UL", "OL"]);

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Plain text (a signature, a quoted reply) as editor lines, one block per line. */
export function textToHtml(text: string): string {
  if (text === "") return "";
  return text
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((line) => (line === "" ? "<div><br></div>" : `<div>${escapeHtml(line)}</div>`))
    .join("");
}

/** The editor's text as plain text: lines kept, list items with a dash, photos as a word. */
export function editorText(root: Node): string {
  let out = "";
  const walk = (node: Node) => {
    node.childNodes.forEach((child) => {
      if (child.nodeType === Node.TEXT_NODE) {
        out += child.textContent ?? "";
        return;
      }
      if (!(child instanceof Element)) return;
      const tag = child.tagName;
      if (tag === "BR") {
        out += "\n";
        return;
      }
      if (tag === "IMG") {
        out += t("compose.photoInText");
        return;
      }
      const isBlock = BLOCKS.has(tag);
      if (isBlock && out !== "" && !out.endsWith("\n")) out += "\n";
      if (tag === "LI") out += "- ";
      walk(child);
      if (isBlock && !out.endsWith("\n")) out += "\n";
    });
  };
  walk(root);
  return out.endsWith("\n") ? out.slice(0, -1) : out;
}

/** Whether the editor holds anything beyond plain lines. */
export function hasFormatting(root: Element): boolean {
  return root.querySelector(FORMATTING) !== null;
}
