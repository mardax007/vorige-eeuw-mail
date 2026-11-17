// Puts the rescue tool for damaged Outlook files (readpst from libpst, GPL-2.0-or-later) and the
// DLLs it needs into src-tauri/rescue/, which the installer ships as rescue\ next to the app
// (bundle.resources in tauri.conf.json). The files come from MSYS2's ucrt64 package
// mingw-w64-ucrt-x86_64-libpst; the licence of every package they come from is copied along.
//
// Without MSYS2 the folder holds only README.txt and the app simply has no rescue mode, unless
// VEM_RESCUE_REQUIRED=1 (the release workflow), which makes a missing tool an error.
//
//   node scripts/rescue-tool.mjs
//
// MSYS2_ROOT overrides C:\msys64.

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = process.env.MSYS2_ROOT || "C:\\msys64";
const BIN = join(ROOT, "ucrt64", "bin");
const OUT = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "rescue");
const REQUIRED = process.env.VEM_RESCUE_REQUIRED === "1";
const PREFIX = "mingw-w64-ucrt-x86_64-";

function msys(tool, args) {
  return execFileSync(join(ROOT, "usr", "bin", tool), args, {
    encoding: "utf8",
    env: { ...process.env, PATH: `${BIN};${join(ROOT, "usr", "bin")};${process.env.PATH}` },
  });
}

/** The DLLs readpst loads from MSYS2 itself (not from Windows), by ldd. */
function dllsOf(exe) {
  return msys("ldd.exe", [exe])
    .split("\n")
    .map((line) => line.match(/=>\s+\/ucrt64\/bin\/(\S+\.dll)/i)?.[1])
    .filter(Boolean)
    .map((name) => join(BIN, name));
}

/** The package a file belongs to, with its version and home page. */
function packageOf(file) {
  const name = msys("pacman.exe", ["-Qoq", `/ucrt64/bin/${basename(file)}`]).trim();
  const info = msys("pacman.exe", ["-Qi", name]);
  const field = (key) => info.match(new RegExp(`^${key}\\s*:\\s*(.*)$`, "m"))?.[1].trim() ?? "";
  return { name, version: field("Version"), url: field("URL"), licence: field("Licenses") };
}

function readme(packages) {
  const lines = [
    "Rescue tool for Outlook files that Vorige Eeuw Mail cannot read itself.",
    "",
    "readpst.exe is part of libpst (https://github.com/pst-format/libpst), GPL-2.0-or-later.",
    "It and the DLLs beside it are unchanged binaries from the MSYS2 project (https://www.msys2.org).",
    "The licence of every package is in licenses\\<package>\\.",
    "The source of each package, at exactly these versions, is at https://repo.msys2.org/mingw/sources/",
    "and the build recipes at https://github.com/msys2/MINGW-packages.",
    "",
    "Packages:",
    ...packages.map((p) => `  ${p.name} ${p.version}  (${p.licence})  ${p.url}`),
    "",
  ];
  return lines.join("\r\n");
}

// .gitkeep is committed, so that cargo check finds the folder on a fresh checkout.
mkdirSync(OUT, { recursive: true });
for (const name of readdirSync(OUT)) {
  if (name !== ".gitkeep") rmSync(join(OUT, name), { recursive: true, force: true });
}

const exe = join(BIN, "readpst.exe");
if (!existsSync(exe)) {
  writeFileSync(join(OUT, "README.txt"), "No rescue tool in this build (MSYS2's libpst was not installed).\r\n");
  const say = `rescue tool: ${exe} not found; install MSYS2 and run: pacman -S ${PREFIX}libpst`;
  if (REQUIRED) {
    console.error(say);
    process.exit(1);
  }
  console.warn(`${say} (this build has no rescue mode)`);
  process.exit(0);
}

const files = [...new Set([exe, ...dllsOf(exe)])];
const packages = new Map();
for (const file of files) {
  cpSync(file, join(OUT, basename(file)));
  const p = packageOf(file);
  packages.set(p.name, p);
}
for (const p of packages.values()) {
  const base = p.name.startsWith(PREFIX) ? p.name.slice(PREFIX.length) : p.name;
  const licences = join(ROOT, "ucrt64", "share", "licenses", base);
  if (existsSync(licences)) cpSync(licences, join(OUT, "licenses", base), { recursive: true });
  else if (REQUIRED) {
    console.error(`rescue tool: no licence files for ${p.name}`);
    process.exit(1);
  }
}
const sorted = [...packages.values()].sort((a, b) => a.name.localeCompare(b.name));
writeFileSync(join(OUT, "README.txt"), readme(sorted));
console.log(`rescue tool: ${files.length} files from ${packages.size} packages in src-tauri/rescue`);
