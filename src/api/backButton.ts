// The Android back key and back gesture. A screen that shows a "Terug" (an open message over the
// list, the folder screen, a page other than the mail) adds a step while it shows it, and the key
// then does what that Terug does. With no step left the key works as Android's own: the app closes.
// Windows has no back key; nothing is registered there.

import { hasTauri } from "./client";

export type BackStep = () => void;

interface Entry {
  step: BackStep;
  /** Deeper steps win: an open contact (2) before the Contactpersonen page itself (1). */
  depth: number;
  order: number;
}

interface Listener {
  unregister: () => Promise<void>;
}

const entries: Entry[] = [];
let counter = 0;
let listener: Promise<Listener | null> | null = null;

function isAndroid(): boolean {
  return typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
}

/** The step the back key takes now: the deepest, and of those the newest. */
export function currentStep(): BackStep | undefined {
  let best: Entry | undefined;
  for (const e of entries) {
    if (!best || e.depth > best.depth || (e.depth === best.depth && e.order > best.order)) best = e;
  }
  return best?.step;
}

function sync() {
  if (!hasTauri() || !isAndroid()) return;
  if (entries.length > 0 && listener === null) {
    listener = import("@tauri-apps/api/app")
      .then((m) => m.onBackButtonPress(() => currentStep()?.()))
      .catch(() => null);
  } else if (entries.length === 0 && listener !== null) {
    const old = listener;
    listener = null;
    void old.then((l) => l?.unregister()).catch(() => undefined);
  }
}

/** Adds a step; the returned function takes it away again. */
export function pushBackStep(step: BackStep, depth: number): () => void {
  const entry: Entry = { step, depth, order: ++counter };
  entries.push(entry);
  sync();
  return () => {
    const i = entries.indexOf(entry);
    if (i >= 0) entries.splice(i, 1);
    sync();
  };
}
