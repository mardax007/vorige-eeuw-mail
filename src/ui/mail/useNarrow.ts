import { useEffect, useState } from "react";

/** Below this width the open message lies over the list instead of next to it. */
export const NARROW_WIDTH = 1100;
/** From this text size the window counts as narrow too (large text needs the room). */
export const LARGE_TEXT = 24;

export function isNarrowWindow(width: number, textSize: number): boolean {
  return width < NARROW_WIDTH || textSize >= LARGE_TEXT;
}

export function useNarrow(textSize: number): boolean {
  const [width, setWidth] = useState(() => (typeof window === "undefined" ? 1280 : window.innerWidth));
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return isNarrowWindow(width, textSize);
}
