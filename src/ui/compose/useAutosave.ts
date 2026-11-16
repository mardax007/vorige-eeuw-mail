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
  /** Called when the last save, as the screen closes, did not work: the screen can no longer say so itself. */
  onLastSaveFailed?: () => void;
}

export interface Autosave {
  /** Saves now if anything changed since the last save. True when nothing is left unsaved. */
  flush: () => Promise<boolean>;
  /** Stops all saving (while Versturen runs, and after it). */
  stop: () => void;
  /** Saves again after a stop (Versturen did not work). */
  resume: () => void;
}

/** Keeps a message being written in Concepten: every few seconds, and when it closes. */
export function useAutosave({ version, hasContent, intervalMs, save, onLastSaveFailed }: AutosaveOptions): Autosave {
  const latest = useRef({ version, hasContent, save, onLastSaveFailed });
  const savedVersion = useRef(0);
  const isStopped = useRef(false);
  const inFlight = useRef<Promise<boolean> | null>(null);

  useEffect(() => {
    latest.current = { version, hasContent, save, onLastSaveFailed };
  });

  const flush = useCallback(async (): Promise<boolean> => {
    // One save at a time: wait for a running one, then look again at what is left.
    while (inFlight.current) await inFlight.current;
    const { version: v, hasContent: has, save: run } = latest.current;
    if (isStopped.current || !has || v === savedVersion.current) return true;
    const attempt = run().then((ok) => {
      if (ok) savedVersion.current = v;
      return ok;
    });
    inFlight.current = attempt;
    try {
      return await attempt;
    } finally {
      if (inFlight.current === attempt) inFlight.current = null;
    }
  }, []);

  const stop = useCallback(() => {
    isStopped.current = true;
  }, []);

  const resume = useCallback(() => {
    isStopped.current = false;
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
      void flush().then((ok) => {
        if (!ok) latest.current.onLastSaveFailed?.();
      });
    };
  }, [flush, intervalMs]);

  return { flush, stop, resume };
}
