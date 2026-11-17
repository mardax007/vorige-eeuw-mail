import { useEffect, useRef } from "react";
import { pushBackStep } from "../../api/backButton";

/**
 * While `isActive`, Android's back key does `onBack` (src/api/backButton.ts). Use the same
 * function as the screen's own "Terug" button. `depth` 1 is a page, 2 something open on a page.
 */
export function useBackStep(isActive: boolean, onBack: () => void, depth: 1 | 2): void {
  const ref = useRef(onBack);
  useEffect(() => {
    ref.current = onBack;
  });
  useEffect(() => {
    if (!isActive) return undefined;
    return pushBackStep(() => ref.current(), depth);
  }, [isActive, depth]);
}
