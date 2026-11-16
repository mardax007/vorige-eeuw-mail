import { useCallback, useEffect, useRef, useState } from "react";
import { copyText } from "./clipboard";

/** How long "Gekopieerd." stays next to its button. */
export const COPY_NOTE_MS = 4000;

export type CopyResult = "copied" | "failed";

export interface CopyNote {
  /** What the last press did, for a few seconds; null otherwise. */
  result: CopyResult | null;
  /** Copies the text; a text that could not be made counts as a failed copy and goes to onError. */
  copy: (text: string | (() => Promise<string>), onError?: (e: unknown) => void) => Promise<void>;
}

/** One copy button's own note: it says whether copying worked, then goes away. */
export function useCopyNote(): CopyNote {
  const [result, setResult] = useState<CopyResult | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const copy = useCallback(async (text: string | (() => Promise<string>), onError?: (e: unknown) => void) => {
    let value: string;
    try {
      value = typeof text === "string" ? text : await text();
    } catch (e: unknown) {
      if (timer.current) clearTimeout(timer.current);
      setResult("failed");
      onError?.(e);
      return;
    }
    const ok = await copyText(value);
    if (timer.current) clearTimeout(timer.current);
    setResult(ok ? "copied" : "failed");
    // A failure stays, since it tells what to do instead.
    if (ok) timer.current = setTimeout(() => setResult(null), COPY_NOTE_MS);
  }, []);

  return { result, copy };
}
