import { describe, expect, it } from "vitest";
import { isNarrowWindow, LARGE_TEXT, NARROW_WIDTH } from "./useNarrow";

describe("isNarrowWindow", () => {
  it("is narrow on a small window or with large text", () => {
    expect(isNarrowWindow(NARROW_WIDTH - 1, 18)).toBe(true);
    expect(isNarrowWindow(1600, LARGE_TEXT)).toBe(true);
    expect(isNarrowWindow(1600, 18)).toBe(false);
  });
});
