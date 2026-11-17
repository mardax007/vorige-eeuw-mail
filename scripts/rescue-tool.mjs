// Puts the rescue tool for damaged Outlook files (readpst from libpst, GPL-2.0-or-later) and the
// DLLs it needs into src-tauri/rescue/, which the installer ships as rescue\ next to the app
// (bundle.resources in tauri.conf.json). The files come from MSYS2's ucrt64 package
// mingw-w64-ucrt-x86_64-libpst; the licence of every package they come from is copied along, and
// the release attaches the source of those packages (scripts/rescue-sources.mjs).
//
// Without MSYS2 the folder holds only README.txt and the app simply has no rescue mode, unless
// VEM_RESCUE_REQUIRED=1 (the release workflow), which makes a missing tool an error.
//
//   node scripts/rescue-tool.mjs
//
// MSYS2_ROOT overrides C:\msys64.

import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { PREFIX, READPST, ROOT, SOURCES_URL, rescueTool } from "./msys-packages.mjs";

const OUT = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "rescue");
const REQUIRED = process.env.VEM_RESCUE_REQUIRED === "1";

function readme(packages) {
  const sources = [...new Set(packages.map((p) => p.source))];
  const lines = [
    "Rescue tool for Outlook files that Vorige Eeuw Mail cannot read itself.",
    "",
    "readpst.exe is part of libpst (https://github.com/pst-format/libpst), GPL-2.0-or-later.",
    "It and the DLLs beside it are unchanged binaries from the MSYS2 project (https://www.msys2.org).",
    "The licence of every package is in licenses\\<package>\\.",
    "",
    "The source code of each package, at exactly these versions, is attached to the GitHub release",
    "of this version of Vorige Eeuw Mail, as the files below. They are also at",
    `${SOURCES_URL} and the build recipes at https://github.com/msys2/MINGW-packages.`,
    "",
    "Packages:",
    ...packages.map((p) => `  ${p.name} ${p.version}  (${p.licence})  ${p.url}`),
    "",
    "Source archives:",
    ...sources.map((s) => `  ${s}`),
    "",
  ];
  return lines.join("\r\n");
}

// .gitkeep is committed, so that cargo check finds the folder on a fresh checkout.
mkdirSync(OUT, { recursive: true });
for (const name of readdirSync(OUT)) {
  if (name !== ".gitkeep") rmSync(join(OUT, name), { recursive: true, force: true });
}

const tool = rescueTool();
if (!tool) {
  writeFileSync(join(OUT, "README.txt"), "No rescue tool in this build (MSYS2's libpst was not installed).\r\n");
  const say = `rescue tool: ${READPST} not found; install MSYS2 and run: pacman -S ${PREFIX}libpst`;
  if (REQUIRED) {
    console.error(say);
    process.exit(1);
  }
  console.warn(`${say} (this build has no rescue mode)`);
  process.exit(0);
}

for (const file of tool.files) cpSync(file, join(OUT, basename(file)));
for (const p of tool.packages) {
  const base = p.name.startsWith(PREFIX) ? p.name.slice(PREFIX.length) : p.name;
  const licences = join(ROOT, "ucrt64", "share", "licenses", base);
  if (existsSync(licences)) cpSync(licences, join(OUT, "licenses", base), { recursive: true });
  else if (REQUIRED) {
    console.error(`rescue tool: no licence files for ${p.name}`);
    process.exit(1);
  }
}
writeFileSync(join(OUT, "README.txt"), readme(tool.packages));
console.log(`rescue tool: ${tool.files.length} files from ${tool.packages.length} packages in src-tauri/rescue`);
