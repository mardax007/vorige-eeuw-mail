// Prepares the Android part of a release (phase 5 task 8): the APK under a fixed name and
// android.json, which the app reads to offer "Bijwerken" (crates/vem-core/src/apk_update.rs).
//
//   node scripts/android-release.mjs --apk <signed.apk> --repo <owner/name> --out <folder> [--notes <file>]
//
// The APK must already carry the update key's signature next to it (<apk>.sig, made by
// `tauri signer sign`); the app refuses an APK whose signature does not match. The version comes
// from src-tauri/tauri.conf.json, so the download address points at release v<version>.

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const APK_NAME = "vorige-eeuw-mail.apk";

function args(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.replace(/^--/, "");
    if (!key || argv[i + 1] === undefined) throw new Error(`missing value for ${argv[i]}`);
    out[key] = argv[i + 1];
  }
  return out;
}

/** The android.json the app reads. */
export function androidJson({ version, repo, signature, notes }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`version "${version}" is not x.y.z`);
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error(`repo "${repo}" is not owner/name`);
  if (!signature.trim()) throw new Error("the signature is empty");
  return {
    version,
    notes: notes.trim(),
    url: `https://github.com/${repo}/releases/download/v${version}/${APK_NAME}`,
    signature: signature.trim(),
  };
}

function main() {
  const a = args(process.argv.slice(2));
  for (const need of ["apk", "repo", "out"]) if (!a[need]) throw new Error(`--${need} is required`);
  const sigFile = `${a.apk}.sig`;
  if (!existsSync(a.apk)) throw new Error(`no APK at ${a.apk}`);
  if (!existsSync(sigFile)) throw new Error(`no signature at ${sigFile}; run tauri signer sign first`);
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const { version } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
  const json = androidJson({
    version,
    repo: a.repo,
    signature: readFileSync(sigFile, "utf8"),
    notes: a.notes ? readFileSync(a.notes, "utf8") : "",
  });
  mkdirSync(a.out, { recursive: true });
  copyFileSync(a.apk, join(a.out, APK_NAME));
  writeFileSync(join(a.out, "android.json"), `${JSON.stringify(json, null, 2)}\n`);
  console.log(`android.json for ${version}: ${json.url}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main();
  } catch (e) {
    console.error(`android-release: ${e instanceof Error ? e.message : String(e)}`);
    process.exit(1);
  }
}
