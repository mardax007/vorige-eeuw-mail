import { describe, expect, it } from "vitest";
import { contrastRatio, TAG_PALETTE, tagColor } from "./tags";

describe("tag palette", () => {
  it("has at least 7:1 contrast for every colour", () => {
    for (const c of TAG_PALETTE) {
      expect(contrastRatio(c.background, c.text), c.key).toBeGreaterThanOrEqual(7);
    }
  });

  it("computes known contrast ratios", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrastRatio("#ffffff", "#ffffff")).toBeCloseTo(1, 5);
  });

  it("falls back to a palette colour for an unknown key", () => {
    expect(TAG_PALETTE).toContain(tagColor("does-not-exist"));
  });
});
