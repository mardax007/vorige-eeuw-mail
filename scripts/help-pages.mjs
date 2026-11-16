// Writes one public help page per VEM-… code into docs/help/, from the registry in docs/errors.md,
// plus an index and the remote-help page (phase 4 task 7). Run: node scripts/help-pages.mjs
// GitHub Pages serves docs/ from the main branch, so a search for a code finds its page.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

const registry = readFileSync("docs/errors.md", "utf8")
  .split("\n")
  .filter((line) => line.startsWith("| `VEM-"))
  .map((line) => {
    const [code, sentence, next] = line.split("|").slice(1, 4).map((cell) => cell.trim());
    return { code: code.replaceAll("`", ""), sentence, next };
  });

if (registry.length === 0) throw new Error("no codes found in docs/errors.md");
mkdirSync("docs/help", { recursive: true });

for (const { code, sentence, next } of registry) {
  writeFileSync(
    `docs/help/${code}.md`,
    `# ${code}

Deze melding komt uit **Vorige Eeuw Mail**, het e-mailprogramma van de familie.

**Wat er aan de hand is:** ${sentence}

**Wat u kunt doen:** ${next}

Onder de melding staat soms "De server zei" met Engelse tekst. Dat is het letterlijke antwoord van
uw e-mailprovider. Martijn kan daaraan zien wat er precies mis ging; u hoeft het niet te begrijpen.

Martijn krijgt van deze melding vanzelf bericht. Wilt u hem zelf iets laten weten, gebruik dan de
knop "Kopiëren voor Martijn" en plak de tekst in een e-mail aan hem.

[Alle meldingen](index.md) · [Hulp op afstand](hulp-op-afstand.md)
`,
  );
}

writeFileSync(
  "docs/help/index.md",
  `# Meldingen van Vorige Eeuw Mail

Elke melding in het programma heeft een code die begint met VEM. Klik op de code voor uitleg.

| Code | Wat er aan de hand is |
| --- | --- |
${registry.map(({ code, sentence }) => `| [${code}](${code}.md) | ${sentence} |`).join("\n")}

[Hulp op afstand](hulp-op-afstand.md)
`,
);

writeFileSync(
  "docs/help/hulp-op-afstand.md",
  `# Hulp op afstand

Soms helpt Martijn u via **Snelle hulp** (Quick Assist) van Windows. Hij kijkt dan op uw scherm mee.

- **Martijn belt u eerst zelf.** Hij geeft u dan een code, en die typt u in Snelle hulp in.
- Belt, mailt of sms't iemand anders u met zo'n code, ook iemand die zegt van Microsoft, de bank of
  uw provider te zijn, of vraagt iemand om mee te kijken: hang op en typ niets in. Dat is oplichting.
- Vorige Eeuw Mail vraagt nooit om uw wachtwoord in een e-mail, en Martijn ook niet.

[Alle meldingen](index.md)
`,
);

console.log(`${registry.length} help pages written to docs/help/`);
