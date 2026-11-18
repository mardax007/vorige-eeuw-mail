// Gives a release build its own, ever-higher version number, so installed apps always see it as an
// update (decision 28). The release workflow runs it before building:
//
//   node scripts/set-version.mjs --patch <number>   keeps major.minor from tauri.conf.json
//   node scripts/set-version.mjs <x.y.z>            sets the whole number
//
// It writes the same number into package.json, src-tauri/tauri.conf.json and the [package] version
// of src-tauri/Cargo.toml and crates/vem-core/Cargo.toml, and prints it. Only the build's own copy
// changes; nothing is committed.

import { readFileSync, writeFileSync } from "node:fs";

const JSON_FILES = ["package.json", "src-tauri/tauri.conf.json"];
const CARGO_FILES = ["src-tauri/Cargo.toml", "crates/vem-core/Cargo.toml"];

function nextVersion(args) {
  if (args[0] === "--patch") {
    const patch = Number(args[1]);
    if (!Number.isInteger(patch) || patch < 0) throw new Error(`Not a patch number: ${args[1]}`);
    const current = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;
    const [major, minor] = current.split(".");
    return `${major}.${minor}.${patch}`;
  }
  if (!/^\d+\.\d+\.\d+$/.test(args[0] ?? "")) throw new Error("Usage: set-version.mjs --patch <n> | <x.y.z>");
  return args[0];
}

function setJson(file, version) {
  const text = readFileSync(file, "utf8");
  const updated = text.replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`);
  if (updated === text && !text.includes(`"version": "${version}"`)) throw new Error(`No version in ${file}`);
  writeFileSync(file, updated);
}

function setCargo(file, version) {
  const text = readFileSync(file, "utf8");
  // Only the first version line, the one under [package]; dependency versions stay.
  const updated = text.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`);
  if (!/^version\s*=/m.test(text)) throw new Error(`No package version in ${file}`);
  writeFileSync(file, updated);
}

try {
  const version = nextVersion(process.argv.slice(2));
  for (const file of JSON_FILES) setJson(file, version);
  for (const file of CARGO_FILES) setCargo(file, version);
  console.log(version);
} catch (e) {
  console.error(e.message);
  process.exit(1);
}
