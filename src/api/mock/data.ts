// Made-up Dutch sample data for the mock backend. No real people or addresses.

import type { Account, Contact, Invite, MailWarning, MessageDetail, Settings, Tag } from "../types";
import type { MockFolder, MockState } from "./state";

const SOLCON = "acc-solcon";
const GMAIL = "acc-gmail";
const LOCAL = "acc-local";

export const sampleAccounts: Account[] = [
  {
    id: SOLCON,
    displayName: "Ria Jansen",
    email: "ria.jansen@solcon.nl",
    kind: "imap",
    quota: { usedKb: 1_880_000, limitKb: 2_000_000 },
    signature: "Hartelijke groet,\nRia",
    lastError: null,
    addresses: [],
    signIn: "password",
  },
  {
    id: GMAIL,
    displayName: "Ria Jansen (Gmail)",
    email: "ria.jansen1950@gmail.com",
    kind: "imap",
    quota: { usedKb: 4_200_000, limitKb: 15_000_000 },
    signature: null,
    lastError: null,
    addresses: [],
    signIn: "google",
  },
  {
    id: LOCAL,
    displayName: "Oude mail uit Outlook",
    email: "",
    kind: "local",
    quota: null,
    signature: null,
    lastError: null,
    addresses: [],
    signIn: null,
  },
];

function folder(id: string, accountId: string, name: string, role: MockFolder["role"], path?: string[]): MockFolder {
  return { id, accountId, name, role, path: path ?? [name] };
}

export const sampleFolders: MockFolder[] = [
  folder("f-s-inbox", SOLCON, "INBOX", "inbox"),
  folder("f-s-sent", SOLCON, "Sent", "sent"),
  folder("f-s-trash", SOLCON, "Trash", "trash"),
  folder("f-s-drafts", SOLCON, "Drafts", "drafts"),
  folder("f-s-junk", SOLCON, "Junk", "junk"),
  folder("f-s-vakantie", SOLCON, "Vakantie", null),
  folder("f-s-klein", SOLCON, "Familie/Kleinkinderen", null, ["Familie", "Kleinkinderen"]),
  folder("f-g-inbox", GMAIL, "INBOX", "inbox"),
  folder("f-g-sent", GMAIL, "[Gmail]/Verzonden berichten", "sent"),
  folder("f-g-trash", GMAIL, "[Gmail]/Prullenbak", "trash"),
  folder("f-g-drafts", GMAIL, "[Gmail]/Concepten", "drafts"),
  folder("f-g-junk", GMAIL, "[Gmail]/Spam", "junk"),
  folder("f-g-archive", GMAIL, "[Gmail]/Alle berichten", "archive"),
  folder("f-l-inbox", LOCAL, "Postvak IN", null, ["Postvak IN"]),
  folder("f-l-sent", LOCAL, "Verzonden items", null, ["Verzonden items"]),
];

export const sampleTags: Tag[] = [
  { id: "t-familie", name: "Familie", color: "green" },
  { id: "t-rekeningen", name: "Rekeningen", color: "blue" },
  { id: "t-bewaren", name: "Bewaren", color: "purple" },
];

export const sampleContacts: Contact[] = [
  { id: "c-1", name: "Martijn", emails: ["martijn@voorbeeld.nl"], phone: null, note: "Helpt met de computer" },
  { id: "c-2", name: "Anneke de Vries", emails: ["anneke.devries@voorbeeld.nl"], phone: "06 12345678", note: null },
  { id: "c-3", name: "Kees Jansen", emails: ["kees.jansen@voorbeeld.nl", "kees@werk-voorbeeld.nl"], phone: null, note: "Broer" },
  { id: "c-4", name: "Huisartsenpraktijk De Linde", emails: ["assistente@delinde-voorbeeld.nl"], phone: "0570 123456", note: null },
  { id: "c-5", name: "Sanne Bakker", emails: ["sanne.bakker@voorbeeld.nl"], phone: null, note: "Kleindochter" },
  { id: "c-6", name: "Tennisclub Smash", emails: ["secretariaat@smash-voorbeeld.nl"], phone: null, note: null },
  { id: "c-7", name: "Wim Peters", emails: ["wim.peters@voorbeeld.nl"], phone: null, note: "Buurman" },
  { id: "c-8", name: "Bep Mulder", emails: ["bep.mulder@voorbeeld.nl"], phone: null, note: null },
];

export const sampleSettings: Settings = {
  textSize: 18,
  notifications: true,
  startWithWindows: true,
  undoSendSeconds: 10,
  shrinkPhotos: true,
  reportNoticeShown: false,
  trayNoticeShown: false,
  backgroundMail: false,
};

/** One sample message: folder, sender, subject, body, hours ago, extras. */
interface Row {
  folderId: string;
  from: [string, string];
  subject: string;
  text: string;
  hours: number;
  unread?: boolean;
  important?: boolean;
  tags?: string[];
  html?: string;
  remote?: boolean;
  attach?: [string, number, string];
  warnings?: MailWarning[];
  invite?: Invite;
  /** Sent to this address instead of the mailbox's own. */
  to?: [string, string];
}

const rows: Row[] = [
  { folderId: "f-s-inbox", from: ["Sanne Bakker", "sanne.bakker@voorbeeld.nl"], subject: "Foto's van de verjaardag", text: "Lieve oma,\n\nHier zijn de foto's van zaterdag. Wat was het gezellig!\n\nLiefs, Sanne", hours: 1, unread: true, tags: ["t-familie"], attach: ["verjaardag.jpg", 2_400_000, "image/jpeg"] },
  { folderId: "f-s-inbox", from: ["Neef Bram", "bram@voorbeeld.nl"], to: ["Familie Jansen", "familie@jansen-voorbeeld.nl"], subject: "Familieweekend in mei", text: "Hallo allemaal,\n\nHet familieweekend is van 15 tot 17 mei. Wie komt er?\n\nBram", hours: 30, unread: true },
  { folderId: "f-s-inbox", from: ["Anneke de Vries", "anneke.devries@voorbeeld.nl"], subject: "Koffie donderdag?", text: "Hoi Ria,\n\nZullen we donderdag weer koffie drinken bij De Molen? Om tien uur?\n\nGroetjes, Anneke", hours: 3, unread: true },
  { folderId: "f-s-inbox", from: ["Energiebedrijf Voorbeeld", "klantenservice@energie-voorbeeld.nl"], subject: "Uw jaarnota staat klaar", text: "Geachte mevrouw Jansen,\n\nUw jaarnota staat klaar in Mijn Omgeving.", hours: 20, tags: ["t-rekeningen"], html: "<h2>Uw jaarnota staat klaar</h2><p>Geachte mevrouw Jansen,</p><p>Uw jaarnota over het afgelopen jaar staat klaar.</p><p><img src=\"https://plaatjes.voorbeeld.invalid/logo.png\" alt=\"Logo\"></p>", remote: true },
  { folderId: "f-s-inbox", from: ["ING Bank", "service@ing-veilig-inloggen.example"], subject: "Dringend: bevestig uw gegevens", text: "Uw rekening wordt geblokkeerd. Klik op de link.", hours: 26, unread: true, html: "<p>Beste klant,</p><p>Uw rekening wordt vandaag geblokkeerd. Log direct in via <a href=\"https://ing-veilig-inloggen.example/login\">www.ing.nl</a>.</p>", warnings: [{ kind: "linkMismatch", shown: "www.ing.nl", actual: "ing-veilig-inloggen.example" }, { kind: "firstContactMoney", shown: null, actual: null }] },
  { folderId: "f-s-inbox", from: ["Kees Jansen", "kees.jansen@voorbeeld.nl"], subject: "Verjaardag tante Truus", text: "Hoi zus,\n\nVergeet je niet dat tante Truus volgende week 90 wordt?\n\nKees", hours: 50, important: true, tags: ["t-familie"] },
  { folderId: "f-s-inbox", from: ["Huisartsenpraktijk De Linde", "assistente@delinde-voorbeeld.nl"], subject: "Herinnering afspraak griepprik", text: "Beste mevrouw Jansen,\n\nU bent welkom voor de griepprik op dinsdag 24 november tussen 14.00 en 16.00 uur.", hours: 75, important: true, attach: ["uitnodiging.ics", 1_200, "text/calendar"], invite: { summary: "Griepprik", start: { wall: "2026-11-24T14:00:00", zone: "Europe/Amsterdam", allDay: false }, end: { wall: "2026-11-24T16:00:00", zone: "Europe/Amsterdam", allDay: false }, location: "Huisartsenpraktijk De Linde, Kerkstraat 4", organizer: { name: "Huisartsenpraktijk De Linde", email: "assistente@delinde-voorbeeld.nl" }, cancelled: false, repeats: false } },
  { folderId: "f-s-inbox", from: ["Tennisclub Smash", "secretariaat@smash-voorbeeld.nl"], subject: "Nieuwsbrief november", text: "Beste leden,\n\nDe baan is vanaf december overdekt.", hours: 98, html: "<h1 style=\"color:#0b3d91\">Nieuwsbrief</h1><p>Beste leden,</p><p>De baan is vanaf december overdekt. <b>Kom gezellig kijken!</b></p><p><img src=\"https://plaatjes.voorbeeld.invalid/baan.jpg\" alt=\"De nieuwe baan\"></p>", remote: true },
  { folderId: "f-s-inbox", from: ["Wim Peters", "wim.peters@voorbeeld.nl"], subject: "Sleutel", text: "Hoi Ria,\n\nIk heb je reservesleutel nog. Zal ik hem morgen langsbrengen?\n\nWim", hours: 120 },
  { folderId: "f-s-inbox", from: ["Gemeente Voorbeeld", "info@gemeente-voorbeeld.nl"], subject: "Afvalkalender 2027", text: "Geachte inwoner,\n\nIn de bijlage vindt u de afvalkalender voor 2027.", hours: 160, tags: ["t-bewaren"], attach: ["afvalkalender-2027.pdf", 380_000, "application/pdf"] },
  { folderId: "f-s-inbox", from: ["Bep Mulder", "bep.mulder@voorbeeld.nl"], subject: "Recept appeltaart", text: "Ria,\n\nHier het recept dat je vroeg:\n- 6 appels\n- 300 gram bloem\n- 200 gram boter\n\nBep", hours: 200 },
  { folderId: "f-s-inbox", from: ["Sanne Bakker", "sanne.bakker@voorbeeld.nl"], subject: "Kerst bij ons?", text: "Oma, kom je met kerst bij ons eten? Mama vraagt het.", hours: 260, tags: ["t-familie"] },
  { folderId: "f-s-inbox", from: ["Zorgverzekeraar Voorbeeld", "post@zorg-voorbeeld.nl"], subject: "Uw nieuwe polis", text: "Geachte mevrouw Jansen,\n\nUw polis voor volgend jaar staat klaar.", hours: 340, tags: ["t-rekeningen", "t-bewaren"] },
  { folderId: "f-s-inbox", from: ["Anneke de Vries", "anneke.devries@voorbeeld.nl"], subject: "Leuke middag gehad", text: "Wat was het fijn vandaag. Tot snel!", hours: 420 },
  { folderId: "f-s-inbox", from: ["Bibliotheek Voorbeeld", "noreply@bieb-voorbeeld.nl"], subject: "Uw boek moet terug", text: "Het boek 'De Avonden' moet over drie dagen terug.", hours: 500 },
  { folderId: "f-s-sent", from: ["Ria Jansen", "ria.jansen@solcon.nl"], subject: "Re: Koffie donderdag?", text: "Prima, tot donderdag!\n\nRia", hours: 2 },
  { folderId: "f-s-sent", from: ["Ria Jansen", "ria.jansen@solcon.nl"], subject: "Dank je wel", text: "Lieve Sanne, dank je wel voor de foto's!", hours: 30 },
  { folderId: "f-s-sent", from: ["Ria Jansen", "ria.jansen@solcon.nl"], subject: "Re: Sleutel", text: "Graag Wim, dank je.", hours: 118 },
  { folderId: "f-s-trash", from: ["Webwinkel Voorbeeld", "aanbiedingen@winkel-voorbeeld.nl"], subject: "Alleen vandaag: 50% korting", text: "Mis het niet!", hours: 40 },
  { folderId: "f-s-trash", from: ["Loterij Voorbeeld", "info@loterij-voorbeeld.nl"], subject: "U hebt gewonnen!", text: "Klik hier om uw prijs op te halen.", hours: 90 },
  { folderId: "f-s-junk", from: ["Onbekend", "prijs@verdacht.example"], subject: "Gefeliciteerd!!!", text: "U hebt een iPhone gewonnen.", hours: 10, unread: true },
  { folderId: "f-s-vakantie", from: ["Reisbureau Voorbeeld", "boekingen@reis-voorbeeld.nl"], subject: "Uw reis naar Texel", text: "Beste mevrouw Jansen,\n\nHierbij de bevestiging van uw reis naar Texel.", hours: 700, attach: ["reisbevestiging.pdf", 120_000, "application/pdf"] },
  { folderId: "f-s-vakantie", from: ["Kees Jansen", "kees.jansen@voorbeeld.nl"], subject: "Foto's Texel", text: "Hier de foto's van onze fietstocht.", hours: 650, attach: ["texel.jpg", 3_100_000, "image/jpeg"] },
  { folderId: "f-s-klein", from: ["Sanne Bakker", "sanne.bakker@voorbeeld.nl"], subject: "Mijn rapport", text: "Oma, ik heb een 8 voor rekenen!", hours: 900, tags: ["t-familie"] },
  { folderId: "f-s-klein", from: ["Daan Bakker", "daan.bakker@voorbeeld.nl"], subject: "Tekening voor oma", text: "Ik heb een tekening voor je gemaakt.", hours: 950, attach: ["tekening.png", 800_000, "image/png"] },
  { folderId: "f-g-inbox", from: ["Google", "no-reply@accounts.google.com"], subject: "Beveiligingsmelding", text: "Er is ingelogd op een nieuw apparaat.", hours: 5, unread: true },
  { folderId: "f-g-inbox", from: ["Kees Jansen", "kees@werk-voorbeeld.nl"], subject: "Fietstocht zondag", text: "Hoi Ria, zin om zondag mee te fietsen? We vertrekken om 11 uur.", hours: 8, unread: true },
  { folderId: "f-g-inbox", from: ["Fotoservice Voorbeeld", "info@foto-voorbeeld.nl"], subject: "Uw fotoboek is verzonden", text: "Uw fotoboek is onderweg.", hours: 45, html: "<p>Goed nieuws!</p><p>Uw fotoboek <i>Zomer 2026</i> is onderweg en komt morgen aan.</p><table border=\"1\" cellpadding=\"6\"><tr><td>Bestelnummer</td><td>123456</td></tr><tr><td>Bezorging</td><td>Morgen</td></tr></table>" },
  { folderId: "f-g-inbox", from: ["Marja Visser", "marja.visser@voorbeeld.nl"], subject: "Reünie klas 1968", text: "Beste Ria,\n\nWe organiseren een reünie van onze oude klas. Kom je ook?\n\nMarja", hours: 130, important: true },
  { folderId: "f-g-inbox", from: ["Webwinkel Voorbeeld", "nieuws@winkel-voorbeeld.nl"], subject: "Nieuwe collectie", text: "Bekijk de nieuwe wintercollectie.", hours: 180, html: "<p>Bekijk de nieuwe <a href=\"https://winkel-voorbeeld.nl/winter\">wintercollectie</a>.</p><p><img src=\"https://plaatjes.voorbeeld.invalid/jas.jpg\" alt=\"Jas\"></p>", remote: true },
  { folderId: "f-g-inbox", from: ["Sanne Bakker", "sanne.bakker@voorbeeld.nl"], subject: "Filmpje van de hond", text: "Kijk eens hoe gek Bobby doet!", hours: 300, tags: ["t-familie"] },
  { folderId: "f-g-inbox", from: ["Belastingdienst Voorbeeld", "info@belasting-voorbeeld.nl"], subject: "Voorlopige aanslag", text: "Uw voorlopige aanslag staat klaar.", hours: 600, tags: ["t-rekeningen"] },
  { folderId: "f-g-sent", from: ["Ria Jansen (Gmail)", "ria.jansen1950@gmail.com"], subject: "Re: Reünie klas 1968", text: "Leuk! Ik kom zeker.", hours: 128 },
  { folderId: "f-g-trash", from: ["Nieuwsbrief Voorbeeld", "nieuws@krant-voorbeeld.nl"], subject: "Het nieuws van vandaag", text: "De belangrijkste berichten.", hours: 60 },
  { folderId: "f-g-junk", from: ["Prins", "prins@verdacht.example"], subject: "Erfenis van 4 miljoen", text: "Stuur uw rekeningnummer.", hours: 15 },
  { folderId: "f-g-archive", from: ["Marja Visser", "marja.visser@voorbeeld.nl"], subject: "Adreslijst klas", text: "Hier is de adreslijst.", hours: 1500 },
  { folderId: "f-l-inbox", from: ["Kees Jansen", "kees.jansen@voorbeeld.nl"], subject: "Gelukkig nieuwjaar 2012", text: "Het beste voor het nieuwe jaar!", hours: 130_000 },
  { folderId: "f-l-inbox", from: ["Henk Jansen", "henk.jansen@voorbeeld.nl"], subject: "Vakantieadres Frankrijk", text: "We zitten op camping Les Pins, plek 12.", hours: 140_000, important: true, tags: ["t-bewaren"] },
  { folderId: "f-l-inbox", from: ["Anneke de Vries", "anneke.devries@voorbeeld.nl"], subject: "Breipatroon", text: "Hier het patroon van de sjaal.", hours: 150_000, attach: ["patroon.doc", 45_000, "application/msword"] },
  { folderId: "f-l-sent", from: ["Ria Jansen", "ria.jansen@home-voorbeeld.nl"], subject: "Aankomst", text: "We zijn goed aangekomen.", hours: 139_000 },
  { folderId: "f-l-sent", from: ["Ria Jansen", "ria.jansen@home-voorbeeld.nl"], subject: "Re: Breipatroon", text: "Dank je wel Anneke!", hours: 149_000 },
];

const HOUR_MS = 3_600_000;

function accountOfFolder(folderId: string): Account {
  const f = sampleFolders.find((x) => x.id === folderId);
  return sampleAccounts.find((a) => a.id === f?.accountId) ?? sampleAccounts[0];
}

function toMessage(row: Row, index: number, now: number): MessageDetail {
  const account = accountOfFolder(row.folderId);
  const toAddr = row.to
    ? [{ name: row.to[0], email: row.to[1] }]
    : row.folderId.endsWith("sent")
    ? [{ name: "Anneke de Vries", email: "anneke.devries@voorbeeld.nl" }]
    : [{ name: account.displayName, email: account.email || "ria.jansen@home-voorbeeld.nl" }];
  const attachments = row.attach
    ? [{ id: `a-${index}`, name: row.attach[0], size: row.attach[1], mime: row.attach[2] }]
    : [];
  return {
    id: `m-${index + 1}`,
    accountId: account.id,
    folderId: row.folderId,
    from: { name: row.from[0], email: row.from[1] },
    to: toAddr,
    subject: row.subject,
    date: new Date(now - row.hours * HOUR_MS).toISOString(),
    preview: row.text.replace(/\s+/g, " ").slice(0, 120),
    unread: row.unread ?? false,
    important: row.important ?? false,
    hasAttachments: attachments.length > 0,
    tagIds: row.tags ?? [],
    cc: [],
    replyTo: [],
    html: row.html ?? null,
    text: row.text,
    hasRemoteImages: row.remote ?? false,
    remoteImagesShown: false,
    attachments,
    warnings: row.warnings ?? [],
    messageIdHeader: `<sample-${index + 1}@voorbeeld.nl>`,
    references: [],
    invite: row.invite ?? null,
  };
}

export function createSampleState(now: number = Date.now()): MockState {
  return {
    accounts: structuredClone(sampleAccounts),
    folders: structuredClone(sampleFolders),
    messages: rows.map((r, i) => toMessage(r, i, now)),
    tags: structuredClone(sampleTags),
    contacts: structuredClone(sampleContacts),
    drafts: {},
    outbox: [],
    undo: {},
    settings: { ...sampleSettings },
    originals: [
      {
        id: "orig-1",
        label: "Outlook.pst",
        size: 1_288_490_188,
        importedAt: new Date(now - 30 * 24 * HOUR_MS).toISOString(),
      },
    ],
    lastImport: null,
    phonePaired: false,
  };
}
