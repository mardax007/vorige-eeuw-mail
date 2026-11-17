import { describe, expect, it } from "vitest";
import { isNarrowWindow, isPhoneWindow, LARGE_TEXT, NARROW_WIDTH, PHONE_WIDTH } from "./useNarrow";

describe("isNarrowWindow", () => {
  it("is narrow on a small window or with large text", () => {
    expect(isNarrowWindow(NARROW_WIDTH - 1, 18)).toBe(true);
    expect(isNarrowWindow(1600, LARGE_TEXT)).toBe(true);
    expect(isNarrowWindow(1600, 18)).toBe(false);
  });
});

describe("isPhoneWindow", () => {
  it("is a phone below the phone width only", () => {
    expect(isPhoneWindow(375)).toBe(true);
    expect(isPhoneWindow(PHONE_WIDTH - 1)).toBe(true);
    expect(isPhoneWindow(PHONE_WIDTH)).toBe(false);
  });
});
