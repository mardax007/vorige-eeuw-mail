// Made-up people, folders and mail for the local test server (scripts/seed-greenmail.mjs).
// Everything is invented and every domain ends in .test, so nothing here can reach a real person.
// The same seed gives the same mail every time.

import { deflateSync } from "node:zlib";

/** A small fast generator with a fixed seed (mulberry32). */
export function makeRandom(seed) {
  let a = seed >>> 0;
  const next = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    next,
    int: (min, max) => min + Math.floor(next() * (max - min + 1)),
    pick: (list) => list[Math.floor(next() * list.length)],
    chance: (p) => next() < p,
  };
}

const FIRST = [
  "Anna", "Bep", "Cor", "Dirk", "Els", "Frans", "Gerda", "Henk", "Ineke", "Joop", "Kees", "Lies",
  "Marja", "Nel", "Otto", "Piet", "Riet", "Sjaak", "Truus", "Wim", "Ans", "Bert", "Corrie", "Dini",
  "Ewout", "Fenna", "Geert", "Hanneke", "Jan", "Karin", "Leo", "Mieke", "Niels", "Paula", "Rob",
  "Sanne", "Thijs", "Ursula", "Vera", "Willem", "Yvonne", "Zeger", "Daan", "Emma", "Lotte", "Sem",
  "Noor", "Finn", "Julia", "Lucas", "Tess", "Bram", "Femke", "Jeroen", "Marloes", "Ruud", "Hennie",
];
const LAST = [
  "Jansen", "de Vries", "van den Berg", "Bakker", "Visser", "Smit", "Meijer", "de Boer", "Mulder",
  "de Groot", "Bos", "Vos", "Peters", "Hendriks", "van Leeuwen", "Dekker", "Brouwer", "de Wit",
  "Dijkstra", "Smits", "de Graaf", "van der Meer", "van der Linden", "Kok", "Jacobs", "de Haan",
  "Vermeulen", "van Dijk", "Schouten", "van Beek", "Willems", "Hoekstra", "Koster", "Prins", "Huisman",
];
const MAIL_DOMAINS = ["kpn.test", "ziggo.test", "hotmail.test", "gmail.test", "solcon.test", "xs4all.test", "planet.test", "home.test"];

/** Shops, clubs and offices that send mail; all invented. */
export const SENDERS = [
  { name: "Energie Noord", email: "klantenservice@energienoord.test", kind: "bill", label: "Rekeningen" },
  { name: "Waterbedrijf Oost", email: "facturen@waterbedrijfoost.test", kind: "bill", label: "Rekeningen" },
  { name: "Bank Zeeland", email: "berichten@bankzeeland.test", kind: "bank", label: "Rekeningen" },
  { name: "Apotheek De Linde", email: "info@apotheekdelinde.test", kind: "health", label: "Gezondheid" },
  { name: "Huisartsenpraktijk Het Plein", email: "assistente@hetplein.test", kind: "health", label: "Gezondheid" },
  { name: "Parochie Sint-Jan", email: "secretariaat@sintjan.test", kind: "news", label: null },
  { name: "Tuinclub De Groene Vingers", email: "bestuur@groenevingers.test", kind: "news", label: null },
  { name: "Breiclub Wol & Zo", email: "ria@wolenzo.test", kind: "news", label: null },
  { name: "Reisbureau Zonnig", email: "boekingen@reisbureauzonnig.test", kind: "travel", label: "Vakantie" },
  { name: "Gemeente Dorpstad", email: "noreply@dorpstad.test", kind: "official", label: "Rekeningen" },
  { name: "Krant van het Noorden", email: "nieuwsbrief@krantnoord.test", kind: "newsletter", label: null },
  { name: "Supermarkt Vers", email: "aanbiedingen@supervers.test", kind: "newsletter", label: null },
  { name: "Bibliotheek Dorpstad", email: "uitlenen@bibdorpstad.test", kind: "news", label: null },
  { name: "Verzekeraar Zeker", email: "polis@verzekeraarzeker.test", kind: "bill", label: "Rekeningen" },
];

function emailFor(first, last, domain, r) {
  const clean = (s) => s.toLowerCase().replace(/[^a-z]/g, "");
  const forms = [
    `${clean(first)}.${clean(last)}`,
    `${clean(first)}${clean(last)}`,
    `${clean(first)[0]}${clean(last)}`,
    `${clean(first)}${r.int(40, 99)}`,
  ];
  return `${r.pick(forms)}@${domain}`;
}

/** `count` different people with Dutch names, a phone number and a group. */
export function makePeople(r, count) {
  const people = [];
  const used = new Set();
  while (people.length < count) {
    const first = r.pick(FIRST);
    const last = r.pick(LAST);
    const email = emailFor(first, last, r.pick(MAIL_DOMAINS), r);
    if (used.has(email)) continue;
    used.add(email);
    people.push({
      name: `${first} ${last}`,
      first,
      email,
      phone: `06-${r.int(10000000, 99999999)}`,
      group: r.pick(["Familie", "Familie", "Kleinkinderen", "Buren", "Vrienden", "Kerk", "Tuin", "Breiclub"]),
    });
  }
  return people;
}

/** The test mailboxes. Folder names follow each provider's own style; "/" is the hierarchy. */
export const ACCOUNTS = [
  {
    email: "oma@solcon.test",
    name: "Oma Jansen",
    style: "Solcon",
    messages: 1200,
    sent: "Sent",
    drafts: "Drafts",
    trash: "Trash",
    junk: "Spam",
    folders: [
      "Familie", "Familie/Kinderen", "Familie/Kleinkinderen", "Vakantie", "Vakantie/2024 Texel",
      "Vakantie/2025 Ardennen", "Rekeningen", "Rekeningen/Energie", "Rekeningen/Bank", "Kerk",
      "Recepten", "Gezondheid", "Oude mail", "Oude mail/2019", "Oude mail/2020",
    ],
  },
  {
    email: "opa.devries@gmail.test",
    name: "Opa de Vries",
    style: "Gmail",
    messages: 900,
    sent: "[Gmail]/Verzonden berichten",
    drafts: "[Gmail]/Concepten",
    trash: "[Gmail]/Prullenbak",
    junk: "[Gmail]/Spam",
    folders: ["[Gmail]/Alle berichten", "Klusjes", "Fietsclub", "Belasting", "Belasting/2024", "Belasting/2025"],
  },
  {
    email: "tante.ria@outlook.test",
    name: "Ria Bakker",
    style: "Outlook",
    messages: 700,
    sent: "Verzonden items",
    drafts: "Concepten",
    trash: "Verwijderde items",
    junk: "Ongewenste e-mail",
    folders: ["Archief", "Breiclub", "Tuin", "Verzekeringen", "Familie", "Familie/Verjaardagen"],
  },
  {
    email: "jan@example.test",
    name: "Jan",
    style: "plain",
    messages: 400,
    sent: "Sent",
    drafts: "Drafts",
    trash: "Trash",
    junk: "Junk",
    folders: ["Werk", "Werk/Projecten", "Hobby"],
  },
  {
    email: "piet@example.test",
    name: "Piet",
    style: "plain",
    messages: 300,
    sent: "Sent",
    drafts: "Drafts",
    trash: "Trash",
    junk: "Junk",
    folders: ["Voetbal", "Verenigingen"],
  },
];

const OPENINGS = ["Hoi", "Hallo", "Lieve", "Beste", "Dag", "Goedemorgen"];
const CLOSINGS = ["Groetjes", "Liefs", "Hartelijke groet", "Tot snel", "Dikke kus", "Met vriendelijke groet"];
const TOPICS = [
  { subject: "Verjaardag van {first} zaterdag", body: "Zaterdag vieren we de verjaardag van {first}. We beginnen om drie uur, koffie en taart staan klaar. Neem je de stoelen mee?", label: "Familie" },
  { subject: "Foto's van het weekend", body: "Hierbij een paar foto's van afgelopen weekend. Wat was het gezellig! De kleintjes vonden het strand prachtig.", label: "Kleinkinderen", photo: true },
  { subject: "Recept appeltaart", body: "Zoals beloofd het recept: 1 kilo goudrenetten, 300 gram bloem, 200 gram roomboter, 150 gram suiker, kaneel en een ei. Een uur in de oven op 175 graden.", label: "Recepten" },
  { subject: "Afspraak bij de dokter", body: "Ik heb donderdag om tien uur een afspraak bij de dokter. Kun jij me brengen? Dan drinken we daarna samen koffie.", label: "Gezondheid" },
  { subject: "Vakantie naar Texel", body: "We hebben een huisje op Texel gevonden voor de eerste week van juni. Er is plek voor zes mensen, dus jullie zijn welkom.", label: "Vakantie" },
  { subject: "Hulp met de computer", body: "Mijn computer doet weer raar. Het scherm wordt steeds zwart als ik een foto open. Kun je een keer kijken als je in de buurt bent?", label: "Klusjes" },
  { subject: "Kerstdiner", body: "Wat zullen we dit jaar met de kerst eten? Ik dacht aan gourmetten, dan kan iedereen kiezen wat hij lekker vindt.", label: "Familie" },
  { subject: "De tuin", body: "De rozen staan prachtig in bloei. De tomaten doen het dit jaar ook goed, ik heb er al een hele schaal van geplukt.", label: null },
  { subject: "Kleinkinderen logeren", body: "Mogen de kinderen in de herfstvakantie twee nachtjes komen logeren? Ze vragen er steeds naar.", label: "Kleinkinderen" },
  { subject: "Bedankt!", body: "Hartelijk bedankt voor het mooie cadeau. Het staat al in de kast en iedereen die binnenkomt ziet het meteen.", label: null },
  { subject: "Nieuws uit de straat", body: "De buren op nummer twaalf gaan verhuizen naar Zeeland. Er komt een jong gezin met twee kinderen voor in de plaats.", label: null },
  { subject: "Fietstocht zondag", body: "Zondag fietsen we een rondje langs de rivier, ongeveer veertig kilometer. Onderweg stoppen we bij het theehuis.", label: null },
  { subject: "Kapotte kraan in de keuken", body: "De kraan in de keuken lekt weer. Heb jij nog zo'n rubbertje liggen? Anders haal ik er morgen een bij de bouwmarkt.", label: "Klusjes" },
  { subject: "Oppassen woensdag", body: "Kun jij woensdagmiddag op de kleine passen? Ik moet naar de tandarts en het is om half drie.", label: "Kleinkinderen" },
];

const BILL_TEXTS = [
  "Uw jaarafrekening staat voor u klaar. U vindt de nota als bijlage bij deze e-mail.",
  "Het termijnbedrag voor de komende maand wordt rond de 25e van uw rekening afgeschreven.",
  "Wij hebben uw betaling in goede orde ontvangen. Hartelijk dank.",
];

function fill(text, vars) {
  return text.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "");
}

function rfc2047(text) {
  return /^[\x20-\x7e]*$/.test(text) ? text : `=?UTF-8?B?${Buffer.from(text, "utf8").toString("base64")}?=`;
}

function addr(person) {
  return `${rfc2047(person.name)} <${person.email}>`;
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const pad = (n) => String(n).padStart(2, "0");

/** RFC 5322 date and the IMAP APPEND internal date for the same moment, in +0100. */
export function dates(ms) {
  const d = new Date(ms + 3600_000);
  const hms = `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}`;
  return {
    header: `${DAYS[d.getUTCDay()]}, ${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()} ${hms} +0100`,
    internal: `${pad(d.getUTCDate())}-${MONTHS[d.getUTCMonth()]}-${d.getUTCFullYear()} ${hms} +0100`,
  };
}

function wrap(b64) {
  return b64.replace(/.{1,76}/g, "$&\r\n");
}

/** A small solid-colour PNG, made here so no image file is needed. */
export function makePng(width, height, [red, green, blue]) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf) => {
    let c = 0xffffffff;
    for (const byte of buf) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 2; // colour type RGB
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array.from({ length: width }, () => [red, green, blue]).flat())]);
  const raw = Buffer.concat(Array.from({ length: height }, () => row));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/** A one-page PDF with one line of text. */
export function makePdf(line) {
  const text = line.replace(/[()\\]/g, "");
  const stream = `BT /F1 18 Tf 72 760 Td (${text}) Tj ET`;
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
  ];
  let out = "%PDF-1.4\n";
  const offsets = [];
  objects.forEach((body, i) => {
    offsets.push(out.length);
    out += `${i + 1} 0 obj\n${body}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  out += offsets.map((o) => `${String(o).padStart(10, "0")} 00000 n \n`).join("");
  out += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(out, "latin1");
}

function base64Part(type, data, extra = []) {
  return [`Content-Type: ${type}`, "Content-Transfer-Encoding: base64", ...extra, "", wrap(data.toString("base64"))].join("\r\n");
}

/** Builds one RFC 5322 message. `attachments` are { name, type, data }. */
export function buildMessage({ from, to, cc = [], subject, date, messageId, inReplyTo, references, text, html, attachments = [] }) {
  const head = [
    `From: ${addr(from)}`,
    `To: ${to.map(addr).join(", ")}`,
    ...(cc.length ? [`Cc: ${cc.map(addr).join(", ")}`] : []),
    `Subject: ${rfc2047(subject)}`,
    `Date: ${date}`,
    `Message-ID: <${messageId}>`,
    ...(inReplyTo ? [`In-Reply-To: <${inReplyTo}>`] : []),
    ...(references?.length ? [`References: ${references.map((ref) => `<${ref}>`).join(" ")}`] : []),
    "MIME-Version: 1.0",
  ];
  const tag = messageId.replace(/[^a-z0-9]/gi, "").slice(0, 24);
  let part = base64Part("text/plain; charset=utf-8", Buffer.from(text, "utf8"));
  if (html) {
    const alt = `alt-${tag}`;
    const htmlPart = base64Part("text/html; charset=utf-8", Buffer.from(html, "utf8"));
    part = `Content-Type: multipart/alternative; boundary="${alt}"\r\n\r\n--${alt}\r\n${part}\r\n--${alt}\r\n${htmlPart}\r\n--${alt}--\r\n`;
  }
  if (attachments.length) {
    const mixed = `mix-${tag}`;
    const files = attachments.map((a) =>
      base64Part(`${a.type}; name="${a.name}"`, a.data, [`Content-Disposition: attachment; filename="${a.name}"`]),
    );
    part = `Content-Type: multipart/mixed; boundary="${mixed}"\r\n\r\n${[part, ...files].map((p) => `--${mixed}\r\n${p}\r\n`).join("")}--${mixed}--\r\n`;
  }
  return Buffer.from(`${head.join("\r\n")}\r\n${part}`, "utf8");
}

/** A personal message body in Dutch, with an optional quote of the message it answers. */
export function personalText(r, to, from, body, quoted) {
  const lines = [`${r.pick(OPENINGS)} ${to.first ?? to.name.split(" ")[0]},`, "", body, "", `${r.pick(CLOSINGS)},`, from.first ?? from.name.split(" ")[0]];
  if (quoted) lines.push("", `Op ${quoted.when} schreef ${quoted.who}:`, ...quoted.text.split("\n").map((l) => `> ${l}`));
  return lines.join("\n");
}

export function topicFor(r) {
  const topic = r.pick(TOPICS);
  const vars = { first: r.pick(FIRST) };
  return { subject: fill(topic.subject, vars), body: fill(topic.body, vars), label: topic.label, photo: topic.photo === true };
}

export function billText(r, sender) {
  return `Beste klant,\n\n${r.pick(BILL_TEXTS)}\n\nKlantnummer: ${r.int(1000000, 9999999)}\n\nMet vriendelijke groet,\n${sender.name}`;
}

/** A newsletter with a remote image (blocked until the user allows it) and ordinary links. */
export function newsletterHtml(sender, issue) {
  const host = sender.email.split("@")[1];
  return `<!doctype html><html><body style="font-family:Arial,sans-serif">
<h1>${sender.name}</h1>
<p><img src="https://images.${host}/nieuwsbrief/${issue}/kop.jpg" alt="Kopfoto" width="600"></p>
<p>Deze week in de aanbieding en het laatste nieuws uit de buurt.</p>
<ul><li>Verse aardbeien voor de halve prijs</li><li>Nieuwe openingstijden in de zomer</li><li>Puzzel van de week</li></ul>
<p><a href="https://www.${host}/nieuws/${issue}">Lees verder op de website</a></p>
<p style="font-size:11px;color:#777">U ontvangt deze nieuwsbrief omdat u zich hebt aangemeld. <a href="https://www.${host}/afmelden">Afmelden</a></p>
</body></html>`;
}

/** A phishing-style mail: the link text shows the bank, the link goes elsewhere. */
export function phishingHtml() {
  return `<!doctype html><html><body style="font-family:Arial,sans-serif">
<p>Geachte klant,</p>
<p>Uw betaalpas verloopt binnenkort. Vernieuw uw pas vandaag nog via onderstaande link, anders wordt uw rekening geblokkeerd.</p>
<p><a href="http://bankzeeland-veilig.verify-now.test/login">https://www.bankzeeland.test/mijnpas</a></p>
<p>Met vriendelijke groet,<br>Bank Zeeland Klantenservice</p>
</body></html>`;
}

/** One vCard 3.0 card. */
export function vcard(person) {
  const [first, ...rest] = person.name.split(" ");
  return [
    "BEGIN:VCARD",
    "VERSION:3.0",
    `N:${rest.join(" ")};${first};;;`,
    `FN:${person.name}`,
    `EMAIL;TYPE=INTERNET:${person.email}`,
    `TEL;TYPE=CELL:${person.phone}`,
    `CATEGORIES:${person.group}`,
    "END:VCARD",
    "",
  ].join("\r\n");
}
