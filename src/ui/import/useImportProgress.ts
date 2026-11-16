import { useCallback, useEffect, useRef, useState } from "react";
import { api, onEvent } from "../../api/client";
import type { ImportProgress } from "../../api/types";

export interface ImportProgressState {
  /** The running import, or the last finished one; null when there was none. */
  progress: ImportProgress | null;
  isRunning: boolean;
  /** Called with the job id start_import or import_from_account gave. */
  begin: (jobId: string) => void;
  /** Asks the core again, for example after it refused a second import. */
  refresh: () => void;
}

function startedJob(jobId: string): ImportProgress {
  return { jobId, done: 0, total: null, currentFolder: null, finished: false, error: null, alreadyThere: 0, skipped: [] };
}

/**
 * Puts an import_status answer next to what is already known. The answer may be older than the
 * events that came in while it was on its way: null never replaces a known import, a finished job
 * is never set back to running, and a job does not go back to fewer messages done.
 */
export function mergeStatus(prev: ImportProgress | null, answer: ImportProgress | null): ImportProgress | null {
  if (answer === null) return prev;
  if (prev === null) return answer;
  if (prev.jobId !== answer.jobId) {
    // The core reports the running job, else the last finished one: a finished other job is older.
    return !prev.finished && answer.finished ? prev : answer;
  }
  if (prev.finished) return prev;
  if (answer.finished) return answer;
  return answer.done >= prev.done ? answer : prev;
}

/**
 * The import's progress, kept by the core rather than by this page: leaving the page and coming
 * back shows a running import again (import_status), and the events keep it up to date.
 */
export function useImportProgress(onFinished: () => void, onError: (e: unknown) => void): ImportProgressState {
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const handlers = useRef({ onFinished, onError });

  useEffect(() => {
    handlers.current = { onFinished, onError };
  });

  const refresh = useCallback(() => {
    api
      .importStatus()
      .then((p) => setProgress((prev) => mergeStatus(prev, p)))
      .catch((e: unknown) => handlers.current.onError(e));
  }, []);

  useEffect(() => {
    let alive = true;
    let stop: (() => void) | null = null;
    // Listen first, so no progress is missed while the status is asked.
    onEvent("import:progress", (p) => {
      setProgress(p);
      if (p.finished) handlers.current.onFinished();
    })
      .then((u) => {
        if (alive) stop = u;
        else u();
      })
      .catch((e: unknown) => handlers.current.onError(e));
    api
      .importStatus()
      .then((p) => {
        // An event that came in meanwhile is newer than this answer.
        if (alive) setProgress((prev) => mergeStatus(prev, p));
      })
      .catch((e: unknown) => handlers.current.onError(e));
    return () => {
      alive = false;
      stop?.();
    };
  }, []);

  const begin = useCallback((jobId: string) => {
    setProgress((prev) => (prev?.jobId === jobId ? prev : startedJob(jobId)));
  }, []);

  return { progress, isRunning: progress !== null && !progress.finished, begin, refresh };
}
