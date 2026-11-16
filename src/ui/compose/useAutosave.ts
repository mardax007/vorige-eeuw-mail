import { useCallback, useEffect, useRef } from "react";

/** How often a message being written is saved to Concepten. */
export const AUTOSAVE_MS = 3000;

interface AutosaveOptions {
  /** Goes up on every change the user makes. */
  version: number;
  /** False while there is nothing worth keeping (an empty new message). */
  hasContent: boolean;
  intervalMs: number;
  /** Saves the message; handles its own errors and says whether it worked. */
  save: () => Promise<boolean>;
}

export interface Autosave {
  /** Saves now if anything changed since the last save. */
  flush: () => Promise<void>;
  /** Stops all saving (after Versturen). */
  stop: () => void;
}

/** Keeps a message being written in Concepten: every few seconds, and when it closes. */
export function useAutosave({ version, hasContent, intervalMs, save }: AutosaveOptions): Autosave {
  const latest = useRef({ version, hasContent, save });
  const savedVersion = useRef(0);
  const isStopped = useRef(false);
  const isSaving = useRef(false);

  useEffect(() => {
    latest.current = { version, hasContent, save };
  });

  const flush = useCallback(async () => {
    const { version: v, hasContent: has, save: run } = latest.current;
    if (isStopped.current || isSaving.current || !has || v === savedVersion.current) return;
    isSaving.current = true;
    try {
      if (await run()) savedVersion.current = v;
    } finally {
      isSaving.current = false;
    }
  }, []);

  const stop = useCallback(() => {
    isStopped.current = true;
  }, []);

  useEffect(() => {
    const timer = setInterval(() => void flush(), intervalMs);
    const onHide = () => void flush();
    window.addEventListener("pagehide", onHide);
    window.addEventListener("beforeunload", onHide);
    return () => {
      clearInterval(timer);
      window.removeEventListener("pagehide", onHide);
      window.removeEventListener("beforeunload", onHide);
      // Closing the screen keeps the text: one last save.
      void flush();
    };
  }, [flush, intervalMs]);

  return { flush, stop };
}
