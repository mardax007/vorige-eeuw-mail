// Fills the local test mail server (GreenMail) with a lot of made-up mail: five mailboxes in the
// style of Solcon, Gmail, Outlook and two plain ones, with nested folders, labels (IMAP keywords),
// threads, attachments, newsletters with remote images and a few phishing-style samples. It also
// writes the made-up people as vCard files, for "Contactpersonen importeren".
//
// Only for the local test server: it refuses any host other than this computer (CLAUDE.md, mail
// safety). The data is invented, every domain ends in .test, and the same --seed gives the same mail.
//
//   node scripts/seed-greenmail.mjs [--port 3993] [--password geheim] [--seed 1] [--scale 1]
//                                   [--vcards <folder>] [--only <address>] [--force]
//
// The users must exist on the server; start GreenMail with all five (CLAUDE.md, Commands).
// A mailbox whose Postvak IN already holds mail is skipped unless --force is given.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { connect } from "node:tls";
import {
  ACCOUNTS, SENDERS, billText, buildMessage, dates, makePdf, makePeople, makePng, makeRandom,
  newsletterHtml, personalText, phishingHtml, topicFor, vcard,
} from "./seed-data.mjs";

const LOCAL_HOSTS = new Set(["127.0.0.1", "localhost", "::1"]);
// The last whole hour: mail lies in the past, and the same seed gives the same mail at the same age.
const NOW = Math.floor(Date.now() / 3600_000) * 3600_000;
const YEARS = 3;
const DAY = 86_400_000;

function parseArgs(argv) {
  const out = { host: "127.0.0.1", port: 3993, password: "geheim", seed: 1, scale: 1, vcards: null, only: null, force: false };
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i].replace(/^--/, "");
    if (key === "force") out.force = true;
    else if (key in out) out[key] = argv[++i];
    else throw new Error(`Unknown option ${argv[i]}`);
  }
  out.port = Number(out.port);
  out.seed = Number(out.seed);
  out.scale = Number(out.scale);
  return out;
}

/** A minimal IMAP client: one command at a time, literals sent after the server's "+". */
class Imap {
  constructor(socket) {
    this.socket = socket;
    this.buffer = "";
    this.waiting = null;
    this.tag = 0;
    socket.setEncoding("latin1");
    socket.on("data", (chunk) => {
      this.buffer += chunk;
      this.pump();
    });
  }

  static open(host, port) {
    return new Promise((resolve, reject) => {
      // The test server has a self-made certificate; this script only ever talks to this computer.
      const socket = connect({ host, port, rejectUnauthorized: false }, () => {
        const client = new Imap(socket);
        client.waitFor((line) => line.startsWith("* OK")).then(() => resolve(client), reject);
      });
      socket.on("error", reject);
    });
  }

  waitFor(test) {
    return new Promise((resolve) => {
      this.waiting = { test, lines: [], resolve };
      this.pump();
    });
  }

  pump() {
    let end;
    while (this.waiting && (end = this.buffer.indexOf("\r\n")) >= 0) {
      const line = this.buffer.slice(0, end);
      this.buffer = this.buffer.slice(end + 2);
      const w = this.waiting;
      w.lines.push(line);
      if (w.test(line)) {
        this.waiting = null;
        w.resolve(w.lines);
      }
    }
  }

  async run(command, literal) {
    const tag = `s${++this.tag}`;
    const done = (line) => line.startsWith(`${tag} `);
    if (literal) {
      this.socket.write(`${tag} ${command} {${literal.length}}\r\n`, "latin1");
      const first = await this.waitFor((line) => line.startsWith("+") || done(line));
      if (!first.at(-1).startsWith("+")) throw new Error(`${command.split(" ")[0]} refused: ${first.at(-1)}`);
      this.socket.write(Buffer.concat([literal, Buffer.from("\r\n")]));
    } else {
      this.socket.write(`${tag} ${command}\r\n`, "latin1");
    }
    const lines = await this.waitFor(done);
    const last = lines.at(-1);
    if (!last.startsWith(`${tag} OK`)) throw new Error(`${command.split(" ")[0]} failed: ${last}`);
    return lines;
  }

  close() {
    this.socket.end();
  }
}

const quote = (s) => `"${s.replace(/["\\]/g, "\\$&")}"`;

/** Fills one mailbox. Returns counts per folder. */
async function seedAccount(imap, account, people, r, scale) {
  const list = await imap.run('LIST "" ""');
  const sep = /\(.*?\) "(.)"/.exec(list[0])?.[1] ?? "/";
  const name = (path) => path.split("/").join(sep);

  const folders = [account.sent, account.drafts, account.trash, account.junk, ...account.folders];
  for (const path of folders) {
    // Parents first: "Familie" before "Familie/Kinderen".
    const parts = path.split("/");
    for (let i = 1; i <= parts.length; i++) {
      const box = name(parts.slice(0, i).join("/"));
      await imap.run(`CREATE ${quote(box)}`).catch((e) => {
        if (!/exist/i.test(e.message)) throw e;
      });
    }
  }

  const me = { name: account.name, email: account.email, first: account.name.split(" ")[0] };
  const friends = people.slice(0, 60 + r.int(0, 40));
  const custom = account.folders.filter((f) => !f.startsWith("[Gmail]/Alle"));
  const counts = {};
  let serial = 0;

  const append = async (path, raw, flags, ms) => {
    const box = path === "INBOX" ? "INBOX" : name(path);
    await imap.run(`APPEND ${quote(box)} (${flags.join(" ")}) "${dates(ms).internal}"`, raw);
    counts[path] = (counts[path] ?? 0) + 1;
  };
  const idFor = () => `${++serial}.${account.email.replace("@", ".")}@seed.test`;
  const when = () => NOW - Math.floor(Math.pow(r.next(), 1.6) * YEARS * 365 * DAY) - r.int(0, 3600_000);
  const seenFlag = (ms) => (NOW - ms > 14 * DAY ? r.chance(0.97) : r.chance(0.5)) ? ["\\Seen"] : [];
  const labelFlags = (label, p) => (label && r.chance(p) ? [`$vem_${label}`] : []);
  const otherKeywords = () => (r.chance(0.05) ? ["$label1"] : r.chance(0.05) ? ["NonJunk"] : []);
  const placeFor = (label) => {
    // Some mail is filed by hand into a folder that matches its label.
    const match = custom.find((f) => label && f.split("/").at(-1).toLowerCase().startsWith(label.toLowerCase()));
    if (match && r.chance(0.5)) return match;
    if (custom.length && r.chance(0.2)) return r.pick(custom);
    return "INBOX";
  };

  const total = Math.round(account.messages * scale);
  while (Object.values(counts).reduce((a, b) => a + b, 0) < total) {
    const kind = r.next();
    if (kind < 0.55) {
      // A personal mail, sometimes answered, sometimes a longer thread.
      const friend = r.pick(friends);
      const topic = topicFor(r);
      let ms = when();
      const first = idFor();
      const photos = topic.photo
        ? Array.from({ length: r.int(1, 3) }, (_, i) => ({
            name: `foto-${i + 1}.png`,
            type: "image/png",
            data: makePng(r.int(40, 96), r.int(30, 72), [r.int(40, 230), r.int(40, 230), r.int(40, 230)]),
          }))
        : [];
      const cc = r.chance(0.15) ? [r.pick(friends)] : [];
      const text = personalText(r, me, friend, topic.body);
      const replies = r.chance(0.3) ? r.int(1, 4) : 0;
      const incoming = buildMessage({ from: friend, to: [me], cc, subject: topic.subject, date: dates(ms).header, messageId: first, text, attachments: photos });
      const flags = [...seenFlag(ms), ...labelFlags(topic.label, 0.35), ...otherKeywords(), ...(replies ? ["\\Answered"] : []), ...(r.chance(0.04) ? ["\\Flagged"] : [])];
      await append(placeFor(topic.label), Buffer.from(incoming), flags, ms);
      let previous = { id: first, text, who: friend.name, ms };
      const refs = [first];
      for (let i = 0; i < replies; i++) {
        ms = previous.ms + r.int(1, 48) * 3600_000;
        // A thread that would reach past now simply ends sooner.
        if (ms >= NOW) break;
        const fromMe = i % 2 === 0;
        const id = idFor();
        const body = fromMe ? r.pick(["Wat leuk, ik kom graag!", "Dank je wel, dat is goed.", "Ik bel je vanavond even.", "Prima, tot dan!"]) : r.pick(["Fijn, tot zaterdag.", "Goed zo, dan weet ik dat.", "Dat is goed. Liefs!"]);
        const reply = buildMessage({
          from: fromMe ? me : friend,
          to: [fromMe ? friend : me],
          subject: `Re: ${topic.subject}`,
          date: dates(ms).header,
          messageId: id,
          inReplyTo: previous.id,
          references: [...refs],
          text: personalText(r, fromMe ? friend : me, fromMe ? me : friend, body, { when: dates(previous.ms).header, who: previous.who, text: previous.text }),
        });
        await append(fromMe ? account.sent : "INBOX", reply, fromMe ? ["\\Seen"] : seenFlag(ms), ms);
        refs.push(id);
        previous = { id, text: body, who: fromMe ? me.name : friend.name, ms };
      }
    } else if (kind < 0.7) {
      // A mail sent by the owner that nobody answered.
      const friend = r.pick(friends);
      const topic = topicFor(r);
      const ms = when();
      const raw = buildMessage({ from: me, to: [friend], subject: topic.subject, date: dates(ms).header, messageId: idFor(), text: personalText(r, friend, me, topic.body) });
      await append(account.sent, raw, ["\\Seen", ...labelFlags(topic.label, 0.2)], ms);
    } else if (kind < 0.88) {
      // Bills, the bank, the doctor, clubs: plain text, sometimes a PDF.
      const sender = r.pick(SENDERS.filter((s) => s.kind !== "newsletter"));
      const ms = when();
      const subject = sender.kind === "bill" ? r.pick(["Uw nota", "Uw jaarafrekening", "Betalingsbevestiging"]) : sender.kind === "health" ? "Uw herhaalrecept staat klaar" : sender.kind === "travel" ? "Uw reisbescheiden" : r.pick(["Mededelingen", "Nieuws van de vereniging", "Uitnodiging ledenvergadering"]);
      const pdf = sender.kind === "bill" || sender.kind === "travel" ? r.chance(0.5) : r.chance(0.1);
      const attachments = pdf ? [{ name: `${subject.toLowerCase().replace(/\s+/g, "-")}.pdf`, type: "application/pdf", data: makePdf(`${sender.name} - ${subject}`) }] : [];
      const raw = buildMessage({ from: sender, to: [me], subject, date: dates(ms).header, messageId: idFor(), text: billText(r, sender), attachments });
      const folder = sender.label && r.chance(0.3) ? placeFor(sender.label) : "INBOX";
      await append(folder, raw, [...seenFlag(ms), ...labelFlags(sender.label, 0.5), ...(r.chance(0.03) ? ["\\Flagged"] : [])], ms);
    } else if (kind < 0.96) {
      // Newsletters with a remote image.
      const sender = r.pick(SENDERS.filter((s) => s.kind === "newsletter"));
      const ms = when();
      const issue = r.int(100, 999);
      const raw = buildMessage({ from: sender, to: [me], subject: `Nieuwsbrief week ${r.int(1, 52)}`, date: dates(ms).header, messageId: idFor(), text: "Bekijk deze nieuwsbrief in een programma dat HTML laat zien.", html: newsletterHtml(sender, issue) });
      await append("INBOX", raw, seenFlag(ms), ms);
    } else if (kind < 0.985) {
      // Trash and junk.
      const ms = when();
      const spam = r.chance(0.5);
      const from = spam ? { name: "Prijzenclub", email: `winnaar${r.int(1, 99)}@prijzen-nu.test` } : r.pick(SENDERS);
      const raw = buildMessage({ from, to: [me], subject: spam ? "U heeft gewonnen!!!" : "Oude mededeling", date: dates(ms).header, messageId: idFor(), text: spam ? "Klik hier om uw prijs op te halen." : "Dit bericht is weggegooid." });
      await append(spam ? account.junk : account.trash, raw, ["\\Seen"], ms);
    } else {
      // A draft that was never sent.
      const friend = r.pick(friends);
      const ms = when();
      const raw = buildMessage({ from: me, to: [friend], subject: r.pick(["Nog even over zaterdag", "", "Vraagje"]), date: dates(ms).header, messageId: idFor(), text: "Hoi,\n\nIk wilde nog even vragen of" });
      await append(account.drafts, raw, ["\\Seen", "\\Draft"], ms);
    }
  }

  // The three warnings the reading pane can show, recent and unread so they are easy to find.
  const known = friends[0];
  const samples = [
    { from: { name: "Bank Zeeland", email: "service@bankzeeland-veilig.test" }, subject: "Uw betaalpas verloopt", html: phishingHtml(), text: "Vernieuw uw pas via de link." },
    { from: { name: known.name, email: `${known.first.toLowerCase()}.nieuw@freemail.test` }, subject: "Nieuw adres", text: `Hoi ${me.first},\n\nDit is mijn nieuwe adres. Kun je me snel terugmailen?\n\n${known.first}` },
    { from: { name: "Kleinzoon Daan", email: "daan.noodgeval@freemail.test" }, subject: "Hoi oma, mijn telefoon is kapot", text: "Hoi,\n\nMijn telefoon is kapot en ik moet vandaag een rekening betalen. Kun je 450 euro overmaken naar dit IBAN: NL00TEST0123456789? Ik betaal het morgen terug.\n\nDaan" },
  ];
  for (const [i, s] of samples.entries()) {
    const ms = NOW - (i + 1) * 3 * 3600_000;
    await append("INBOX", buildMessage({ ...s, to: [me], date: dates(ms).header, messageId: idFor() }), [], ms);
  }
  return counts;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!LOCAL_HOSTS.has(args.host)) {
    throw new Error(`Refusing ${args.host}: this script only fills the local test server on this computer.`);
  }
  const r = makeRandom(args.seed);
  const people = makePeople(r, 250);

  if (args.vcards) {
    mkdirSync(args.vcards, { recursive: true });
    writeFileSync(join(args.vcards, "contactpersonen.vcf"), people.map(vcard).join(""));
    writeFileSync(join(args.vcards, "familie.vcf"), people.filter((p) => p.group === "Familie").map(vcard).join(""));
    console.log(`vCards: ${people.length} people in ${args.vcards}`);
  }

  for (const account of ACCOUNTS) {
    if (args.only && account.email !== args.only) continue;
    const imap = await Imap.open(args.host, args.port);
    try {
      await imap.run(`LOGIN ${quote(account.email)} ${quote(args.password)}`);
      const status = await imap.run('STATUS "INBOX" (MESSAGES)');
      const existing = Number(/MESSAGES (\d+)/.exec(status.join(" "))?.[1] ?? 0);
      if (existing > 0 && !args.force) {
        console.log(`${account.email}: Postvak IN already holds ${existing} messages, skipped (--force to add more)`);
        continue;
      }
      const started = Date.now();
      const counts = await seedAccount(imap, account, people, makeRandom(args.seed * 1000 + ACCOUNTS.indexOf(account)), args.scale);
      const sum = Object.values(counts).reduce((a, b) => a + b, 0);
      console.log(`${account.email} (${account.style}): ${sum} messages in ${Object.keys(counts).length} folders, ${((Date.now() - started) / 1000).toFixed(1)} s`);
      for (const [folder, n] of Object.entries(counts).sort()) console.log(`  ${folder}: ${n}`);
      await imap.run("LOGOUT").catch(() => {});
    } finally {
      imap.close();
    }
  }
}

main().catch((e) => {
  console.error(e.message);
  process.exit(1);
});
