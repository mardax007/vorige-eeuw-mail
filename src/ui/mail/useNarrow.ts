import { useEffect, useState } from "react";

/** Below this width the open message lies over the list instead of next to it. */
export const NARROW_WIDTH = 1100;
/** From this text size the window counts as narrow too (large text needs the room). */
export const LARGE_TEXT = 24;
/** Below this width it is a phone: one part of the mail screen at a time (phase 5). */
export const PHONE_WIDTH = 640;

export function isNarrowWindow(width: number, textSize: number): boolean {
  return width < NARROW_WIDTH || textSize >= LARGE_TEXT;
}

export function isPhoneWindow(width: number): boolean {
  return width < PHONE_WIDTH;
}

function useWindowWidth(): number {
  const [width, setWidth] = useState(() => (typeof window === "undefined" ? 1280 : window.innerWidth));
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}

/** How much room the mail screen has: narrow (message over the list) and phone (one part at a time). */
export function useScreenSize(textSize: number): { isNarrow: boolean; isPhone: boolean } {
  const width = useWindowWidth();
  const isPhone = isPhoneWindow(width);
  return { isNarrow: isPhone || isNarrowWindow(width, textSize), isPhone };
}
