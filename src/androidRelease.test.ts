import { describe, expect, it } from "vitest";
import { androidJson, APK_NAME } from "../scripts/android-release.mjs";

// android.json is read by crates/vem-core/src/apk_update.rs (AndroidRelease): version, notes,
// url and signature, and the app downloads only from https://github.com/.
describe("android.json for a release", () => {
  const base = { version: "0.2.0", repo: "owner/vem", signature: "c2ln\n", notes: "- Nieuw\n" };

  it("points at the APK of this version's release on GitHub", () => {
    const json = androidJson(base);
    expect(json).toEqual({
      version: "0.2.0",
      notes: "- Nieuw",
      url: `https://github.com/owner/vem/releases/download/v0.2.0/${APK_NAME}`,
      signature: "c2ln",
    });
  });

  it("refuses a version that is not x.y.z, a bad repository name and an empty signature", () => {
    expect(() => androidJson({ ...base, version: "0.2" })).toThrow();
    expect(() => androidJson({ ...base, repo: "https://evil.example/x" })).toThrow();
    expect(() => androidJson({ ...base, signature: " \n" })).toThrow();
  });
});
