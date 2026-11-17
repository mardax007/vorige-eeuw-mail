// Fetches the source code of every MSYS2 package the rescue tool is made of (readpst from libpst,
// GPL-2.0-or-later, and the GPL and LGPL libraries it loads), at exactly the installed versions,
// from MSYS2's source archive. The release workflow attaches these files to every release, so
// whoever receives the installer also has the source of those binaries (decision 23).
//
//   node scripts/rescue-sources.mjs <folder>
//
// MSYS2_ROOT overrides C:\msys64. Stops with an error when the tool or any archive is missing.

import { createWriteStream, mkdirSync } from "node:fs";
import { join } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { PREFIX, READPST, SOURCES_URL, rescueTool } from "./msys-packages.mjs";

const out = process.argv[2];
if (!out) {
  console.error("usage: node scripts/rescue-sources.mjs <folder>");
  process.exit(2);
}

const tool = rescueTool();
if (!tool) {
  console.error(`rescue sources: ${READPST} not found; install MSYS2 and run: pacman -S ${PREFIX}libpst`);
  process.exit(1);
}

mkdirSync(out, { recursive: true });
// Several binary packages can come from one source package; fetch each archive once.
const archives = [...new Set(tool.packages.map((p) => p.source))];
for (const name of archives) {
  const url = SOURCES_URL + name;
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    console.error(`rescue sources: ${url} answered ${response.status}`);
    process.exit(1);
  }
  await pipeline(Readable.fromWeb(response.body), createWriteStream(join(out, name)));
  console.log(`rescue sources: ${name}`);
}
console.log(`rescue sources: ${archives.length} archives in ${out}`);
