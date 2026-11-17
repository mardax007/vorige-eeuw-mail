// Types for scripts/android-release.mjs, so the screen tests can check its output.
export declare const APK_NAME: string;
export declare function androidJson(input: {
  version: string;
  repo: string;
  signature: string;
  notes: string;
}): { version: string; notes: string; url: string; signature: string };
