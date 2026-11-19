import { useEffect, useLayoutEffect, useRef, useState, type ClipboardEvent, type MouseEvent } from "react";
import { api } from "../../api/client";
import { t, type MessageKey } from "../../i18n";
import { editorText, hasFormatting } from "./richText";

/** What the editor holds: always the plain text, and the HTML only when it has formatting. */
export interface EditorBody {
  text: string;
  html: string | null;
}

interface RichEditorProps {
  id: string;
  label: string;
  /** Trusted HTML only: a draft cleaned by the core, or plain text through `textToHtml`. */
  initialHtml: string;
  onChange: (body: EditorBody) => void;
  onError: (e: unknown) => void;
}

type Toggle = "bold" | "italic" | "underline" | "insertUnorderedList";

const TOGGLES: readonly { command: Toggle; label: MessageKey; className: string }[] = [
  { command: "bold", label: "compose.bold", className: "format-bold" },
  { command: "italic", label: "compose.italic", className: "format-italic" },
  { command: "underline", label: "compose.underline", className: "format-underline" },
  { command: "insertUnorderedList", label: "compose.list", className: "format-list" },
];

/** The old HTML font sizes: 3 is normal, 5 is clearly larger. */
const SIZE_NORMAL = "3";
const SIZE_BIG = "5";

const COLORS: readonly { label: MessageKey; value: string }[] = [
  { label: "compose.colorBlack", value: "#000000" },
  { label: "compose.colorRed", value: "#c00000" },
  { label: "compose.colorBlue", value: "#1f4e9c" },
  { label: "compose.colorGreen", value: "#2e7d32" },
];

/** execCommand is old, but it is the one editing API that WebView2 and Android's WebView share. */
function run(command: string, value?: string): boolean {
  try {
    return typeof document.execCommand === "function" && document.execCommand(command, false, value);
  } catch {
    return false;
  }
}

function isOn(command: string): boolean {
  try {
    return typeof document.queryCommandState === "function" && document.queryCommandState(command);
  } catch {
    return false;
  }
}

function isBig(): boolean {
  try {
    return typeof document.queryCommandValue === "function" && document.queryCommandValue("fontSize") === SIZE_BIG;
  } catch {
    return false;
  }
}

/** Put a node where the cursor is, for when execCommand is not there. */
function insertAtCursor(node: Node): void {
  const sel = document.getSelection();
  if (!sel || sel.rangeCount === 0) return;
  const range = sel.getRangeAt(0);
  range.deleteContents();
  range.insertNode(node);
  range.setStartAfter(node);
  range.collapse(true);
  sel.removeAllRanges();
  sel.addRange(range);
}

/** Keeps the cursor in the text when a toolbar button is pressed. */
const keepCursor = (e: MouseEvent) => e.preventDefault();

/** The message text with a small toolbar (decision 33): formatting, colour and photos in the text. */
export function RichEditor({ id, label, initialHtml, onChange, onError }: RichEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const saved = useRef<Range | null>(null);
  const [first] = useState(initialHtml);
  const [active, setActive] = useState<{ toggles: Toggle[]; isBig: boolean }>({ toggles: [], isBig: false });
  const [color, setColor] = useState(COLORS[0].value);
  const [problem, setProblem] = useState<string | null>(null);
  const labelId = `${id}-label`;

  useLayoutEffect(() => {
    if (editorRef.current) editorRef.current.innerHTML = first;
  }, [first]);

  const readState = () => setActive({ toggles: TOGGLES.filter((m) => isOn(m.command)).map((m) => m.command), isBig: isBig() });

  // Remember where the cursor was, so a button, the colour list or the photo dialog can put it back.
  useEffect(() => {
    const onSelection = () => {
      const sel = document.getSelection();
      const el = editorRef.current;
      if (!sel || sel.rangeCount === 0 || !el || !el.contains(sel.anchorNode)) return;
      saved.current = sel.getRangeAt(0).cloneRange();
      readState();
    };
    document.addEventListener("selectionchange", onSelection);
    return () => document.removeEventListener("selectionchange", onSelection);
  }, []);

  const emit = () => {
    const el = editorRef.current;
    if (!el) return;
    onChange({ text: editorText(el), html: hasFormatting(el) ? el.innerHTML : null });
  };

  const restoreCursor = () => {
    const el = editorRef.current;
    if (!el) return;
    el.focus();
    const sel = document.getSelection();
    if (!sel) return;
    let range = saved.current;
    if (!range) {
      range = document.createRange();
      range.selectNodeContents(el);
      range.collapse(false);
    }
    sel.removeAllRanges();
    sel.addRange(range);
  };

  const apply = (command: string, value?: string) => {
    restoreCursor();
    run(command, value);
    emit();
    readState();
  };

  // Pasted text comes in as plain text: no outside formatting, scripts or pictures get in.
  const handlePaste = (e: ClipboardEvent) => {
    e.preventDefault();
    const text = e.clipboardData.getData("text/plain");
    if (!text) return;
    if (!run("insertText", text)) insertAtCursor(document.createTextNode(text));
    emit();
  };

  const addPhoto = async () => {
    setProblem(null);
    try {
      const picked = await api.pickTextPhoto(t("compose.photoFilter"));
      if (!picked) return;
      if (!picked.dataUrl) {
        setProblem(t("compose.photoNotUsable"));
        return;
      }
      restoreCursor();
      if (!run("insertImage", picked.dataUrl)) {
        const img = document.createElement("img");
        img.src = picked.dataUrl;
        insertAtCursor(img);
      }
      emit();
    } catch (e: unknown) {
      onError(e);
    }
  };

  return (
    <div className="form-row grow">
      <label id={labelId} onClick={() => editorRef.current?.focus()}>{label}</label>
      <div className="format-bar" role="toolbar" aria-label={t("compose.format")} aria-controls={id}>
        {TOGGLES.map((m) => (
          <button
            key={m.command}
            type="button"
            className={m.className}
            aria-pressed={active.toggles.includes(m.command)}
            onMouseDown={keepCursor}
            onClick={() => apply(m.command)}
          >
            {t(m.label)}
          </button>
        ))}
        <button
          type="button"
          className="format-bigger"
          aria-pressed={active.isBig}
          onMouseDown={keepCursor}
          onClick={() => apply("fontSize", active.isBig ? SIZE_NORMAL : SIZE_BIG)}
        >
          {t("compose.bigger")}
        </button>
        <label className="format-color">
          {t("compose.color")}
          <select
            value={color}
            onChange={(e) => {
              setColor(e.target.value);
              apply("foreColor", e.target.value);
            }}
          >
            {COLORS.map((c) => (
              <option key={c.value} value={c.value}>{t(c.label)}</option>
            ))}
          </select>
        </label>
        <button type="button" onMouseDown={keepCursor} onClick={() => void addPhoto()}>
          {t("compose.photo")}
        </button>
      </div>
      <div
        id={id}
        ref={editorRef}
        className="rich-editor"
        contentEditable
        suppressContentEditableWarning
        role="textbox"
        aria-multiline="true"
        aria-labelledby={labelId}
        spellCheck
        lang="nl"
        onInput={emit}
        onPaste={handlePaste}
        onDrop={(e) => e.preventDefault()}
      />
      {problem && <p className="form-problem" role="alert">{problem}</p>}
    </div>
  );
}
