import { describe, expect, it } from "vitest";

/**
 * CLAUDE.md, Screen text: no hard-coded screen text; every string goes through the catalogue.
 * This reads every screen file and flags words written straight into JSX: text between tags and
 * text in the attributes a person reads or hears (aria-label, title, placeholder, alt).
 */
const SCREENS: Record<string, string> = import.meta.glob(["../**/*.tsx", "!../**/*.test.tsx", "!../test/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

/** Two or more letters in a row: a word, not a separator such as "·" or "–". */
const WORD = /\p{L}{2,}/u;
// Not after "=>": that is an arrow function returning a type such as Promise<void>.
const TEXT_BETWEEN_TAGS = /(?<!=)>([^<>{}]*)[<{]/g;
const SPOKEN_ATTRIBUTE = /\b(aria-label|title|placeholder|alt)="([^"]*)"/g;

function literals(source: string): string[] {
  // Comments may hold any words.
  const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
  const found: string[] = [];
  for (const m of code.matchAll(TEXT_BETWEEN_TAGS)) {
    const text = m[1].trim();
    // "=>" and generics also contain ">"; real JSX text has no code characters in it.
    if (WORD.test(text) && !/[=;()&|]/.test(text)) found.push(text);
  }
  for (const m of code.matchAll(SPOKEN_ATTRIBUTE)) {
    if (WORD.test(m[2])) found.push(`${m[1]}="${m[2]}"`);
  }
  return found;
}

describe("screen text", () => {
  it("finds a literal when there is one", () => {
    expect(literals(`<button aria-label="Sluiten">x</button><p>Hallo daar</p><p>Beste {naam}</p>`)).toEqual([
      "Hallo daar",
      "Beste",
      'aria-label="Sluiten"',
    ]);
    expect(literals(`<p>{t("list.empty")}</p> · {a} – {b}`)).toEqual([]);
  });

  it("has no words written straight into a screen", () => {
    const files = Object.entries(SCREENS);
    expect(files.length).toBeGreaterThan(20);
    const offenders = files.flatMap(([file, source]) => literals(source).map((text) => `${file}: ${text}`));
    expect(offenders).toEqual([]);
  });
});
