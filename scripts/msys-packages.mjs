// What the rescue tool (readpst from MSYS2's ucrt64 libpst) is made of: the executable, the DLLs it
// loads from MSYS2, and the MSYS2 package each file comes from. Shared by rescue-tool.mjs (copies
// the files into the installer) and rescue-sources.mjs (fetches the source of those packages for
// the release).
//
// MSYS2_ROOT overrides C:\msys64.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";

export const ROOT = process.env.MSYS2_ROOT || "C:\\msys64";
export const BIN = join(ROOT, "ucrt64", "bin");
export const PREFIX = "mingw-w64-ucrt-x86_64-";
export const READPST = join(BIN, "readpst.exe");
/** MSYS2's archive of source packages, one per package base and version. */
export const SOURCES_URL = "https://repo.msys2.org/mingw/sources/";

function msys(tool, args) {
  return execFileSync(join(ROOT, "usr", "bin", tool), args, {
    encoding: "utf8",
    env: { ...process.env, PATH: `${BIN};${join(ROOT, "usr", "bin")};${process.env.PATH}` },
  });
}

/** The DLLs an executable loads from MSYS2 itself (not from Windows), by ldd. */
function dllsOf(exe) {
  return msys("ldd.exe", [exe])
    .split("\n")
    .map((line) => line.match(/=>\s+\/ucrt64\/bin\/(\S+\.dll)/i)?.[1])
    .filter(Boolean)
    .map((name) => join(BIN, name));
}

/**
 * The package base a package was built from (gcc-libs comes from mingw-w64-gcc, for example).
 * The source archive is named after it. pacman -Qi does not show it; the local database does.
 */
function baseOf(name, version) {
  const desc = join(ROOT, "var", "lib", "pacman", "local", `${name}-${version}`, "desc");
  if (existsSync(desc)) {
    const base = readFileSync(desc, "utf8").match(/^%BASE%\r?\n(.+)$/m)?.[1].trim();
    if (base) return base;
  }
  return name.startsWith(PREFIX) ? `mingw-w64-${name.slice(PREFIX.length)}` : name;
}

/** The package a file belongs to, with its version, home page, licence and source archive. */
function packageOf(file) {
  const name = msys("pacman.exe", ["-Qoq", `/ucrt64/bin/${basename(file)}`]).trim();
  const info = msys("pacman.exe", ["-Qi", name]);
  const field = (key) => info.match(new RegExp(`^${key}\\s*:\\s*(.*)$`, "m"))?.[1].trim() ?? "";
  const version = field("Version");
  const base = baseOf(name, version);
  return { name, version, url: field("URL"), licence: field("Licenses"), source: `${base}-${version}.src.tar.zst` };
}

/** readpst and its DLLs, and the packages they come from (sorted by name). Null without MSYS2's libpst. */
export function rescueTool() {
  if (!existsSync(READPST)) return null;
  const files = [...new Set([READPST, ...dllsOf(READPST)])];
  const packages = new Map();
  for (const file of files) {
    const p = packageOf(file);
    packages.set(p.name, p);
  }
  return { files, packages: [...packages.values()].sort((a, b) => a.name.localeCompare(b.name)) };
}
