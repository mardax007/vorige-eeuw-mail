# Decisions

Decisions belong to Martijn. Claude Code records them here with the date and never fills in an open
one by itself.

## Decided (16 November 2026)

- The app is free and for the family only. It stays simple, setup included.
- A desktop app, not a web app. The first idea (a web app on Firebase with Cloudflare) is dropped.
- Windows on the desktop, then Android. No macOS, Linux or iPhone.
- A client for existing mailboxes over IMAP and SMTP. Not a mail host.
- Unsigned installers and APKs are fine. No stores, no paid signing, no reviews.
- Import sources: Outlook (PST) and Windows Live Mail from this computer, and any mailbox over IMAP.
- Thunderbird, Eudora and Apple Mail are excluded as import sources.
- Imported mail can go into the new account's mailbox or stay in local folders next to it.
- First case to serve: import old mail and connect a new Solcon account alongside it.
- The error rule in `errors.md`.
- **Search is fast at three levels:** all mail, one mailbox, one folder. Local index only; first
  results within 0.2 s on 100,000 messages (Martijn asked for fast search; he accepted the 0.2 s
  target).
- **Tags and contacts are in version 1** (Martijn). Tags on the server as IMAP keywords or Gmail
  labels where possible; a simple address book on the device. Details in `features.md`. This answers
  the former question 16.
- **All proposals of the research session are accepted** (Martijn: "I like everything you
  proposed"): the **(review)** and **(research)** lines in `PLAN.md`, all of `sync.md` and
  `screens.md`, and the design details in `features.md`. A later rejection is recorded here.
- **Decision 2, screens framework:** React + TypeScript + Vite.
- **Decision 5, language:** the app is in Dutch. All text still goes through one message catalogue,
  so English stays possible later. Where Dutch Outlook has a word the family knows, use it
  (`screens.md`).
- **Decision 10, actions:** delete, move to a folder, mark read or unread are in version 1.
- **Decision 13, better than Thunderbird:** the working answer in `PLAN.md` section 1.
- **Decision 17, secrets:** DPAPI (`CryptProtectData`, current user); the result in its own file in
  the app's data folder. The hard rule in `CLAUDE.md` is changed to match.
- **Decision 20, candidates in version 1:** C1 (smaller photos), C2 (new-mail notice), C3 (undo
  send), C4 (print), C6 (signature), C7 (all inboxes). C12 (start with Windows) comes with them,
  because Windows notifications only help while the app runs (Claude's addition; say so if it is not
  wanted).
- **Network hard rule:** the proposed wording is accepted and is now in `CLAUDE.md`.
- **Wish button** (Martijn): "Wens of idee" sends Martijn an email from the user's own account.
- **Error reports and logs** (Martijn): errors are emailed to Martijn at once; the full log is sent
  when Martijn asks for it from a distance. By email through the user's own account, no server of
  ours. Design in `PLAN.md` section 5.
- **Updates** (Martijn): automatic, silent, no user action. Published through GitHub. Design in
  `PLAN.md` section 6.
- **Windows notifications** (Martijn): new mail shows a Windows notification. The app keeps running
  next to the clock when its window is closed.
- **Screens** (Martijn): simple and plain for older users. Outlook is their reference point, but it
  is not copied: Outlook is what we are replacing because it is hard to use. `screens.md` says what is
  kept and what is dropped.
- **Decision 1, providers** (Martijn): the inventory is unknown, so support as much as possible.
  Every provider with IMAP and SMTP and a password works the same way; Gmail gets its own sign-in
  and labels; Outlook.com gets Microsoft sign-in if the app registration works (decision 9). The
  inventory is still filled in where people know it (import sources, Windows, antivirus), but it no
  longer blocks phase 1. The spike's real-account checks use the accounts Martijn can reach.
- **Decision 3, identifier:** `tech.mardax.vem`, for the Windows app and the Android package.
  Permanent once installed.
- **Decision 4, repository:** one public GitHub repository holds the source, the releases and
  `log-requests.json`. Nothing private goes into it: keys and the report address are GitHub Actions
  secrets.
- **Decision 6, Tauri or Electron:** Martijn leaves it to Claude Code, on the spike report. Tauri
  unless the spike shows it cannot do the job; the report gives the reasons.
- **Decision 7, mail on the computer:** everything. A full offline copy: headers first, then every
  body in the background, newest first, within Gmail's limits.
- **Decision 8, Google sign-in:** yes, in version 1.
- **Decision 9, Outlook.com and Hotmail:** supported if the Microsoft app registration works (part of
  "support as much as possible"). If it cannot be registered, the app says plainly that it cannot
  open Outlook.com mailboxes.
- **Decision 10, actions:** confirmed again by Martijn.
- **Decision 11, Outlook Express (DBX):** yes, import it. Still an unproven format; phase 3 tests the
  readers on real files first.
- **Decision 12, import over the internet:** no. Phase 6 and the relay are dropped. Old mail comes by
  USB stick (exFAT or NTFS for large files) or from a mailbox.
- **Decision 14, licence, install, signing:** GPL-3.0. Installed per user, without admin rights, so
  updates stay silent (an admin install would ask for permission at every update). Unsigned for now;
  the GPL licence and the public repository keep free signing through SignPath Foundation open.
- **Decision 15, large old mail:** it stays on the computer as a local account in our own store
  (`.eml` files plus the SQLite index, the same as synced mail), so it is searched, tagged and shown
  like the rest. The original import file (the PST or the mail folder) is kept as a copy in the app's
  data folder, so nothing is lost if the conversion missed something; a menu lists these copies with
  their size and can delete them (with one confirmation, since that cannot be undone). This copy is
  on the same disk, so it is not a backup against a broken disk; the export remains for that.
- **Mailbox space** (Martijn asked, given as his answer 16): the app shows how full each mailbox is. Read from
  the server with IMAP `QUOTA` where it is offered; otherwise the used space is added up from the
  message sizes and the limit comes from our provider table when known. Warn at 90% full. The import
  checks it before "Put in my mailbox" (already planned).
- **Decision 19, contacts on several devices:** yes. The phone is paired with the PC once by
  scanning a QR code on the PC, which carries a random secret key (and the account to use, never a
  password). Contacts travel encrypted with that key, as one message in a folder of the user's own
  mailbox ("Vorige Eeuw Mail"). No server of ours; Firebase was considered and not chosen, because it
  would be a server of ours to maintain and would change the hard rules. Design in `features.md`.
  Candidate C13 is chosen.
- **Decision 21, report address:** answered by Martijn. It is kept as the GitHub Actions secret
  `VEM_SUPPORT_ADDRESS` and in Martijn's own notes, never in this public repository. It is an address
  made only for this, and receiving it is set up at Martijn's mail domain (Martijn).
- **Decision 22, Android updates:** the app checks GitHub and shows one "Bijwerken" button that opens
  Android's own install screen.

## Open

None. Decision 6 is made (Tauri stays; see "Build choices" below).

## Build choices (16 November 2026, Claude Code; Martijn may overrule any of them)

Made while building phases 1 to 4. None of them changes a hard rule; each is easy to reverse.

- **Decision 6, stack:** Tauri 2 stays. The build proves it: IMAP, SMTP, sync, search, import and the
  shell all run in Rust under Tauri 2.12, and the GreenMail tests pass end to end. The Electron
  fallback is not needed.
- **Own SMTP client instead of `lettre`.** About 400 lines in `vem-core/src/smtp.rs`. It keeps the
  server's exact reply for the error box (`errors.md`), uses the same rustls connection with the
  system certificate store as IMAP, stops when a required STARTTLS is missing, and speaks
  XOAUTH2 for Google and Microsoft. Tested against a fake server and GreenMail.
- **No MX or SRV lookup yet.** Settings come from the provider table, the domain's autoconfig file,
  ISPDB, and a guess under the user's own domain. The MX/SRV step of `PLAN.md` is not built; a
  host found that way would need the confirmation screen, which exists and is tested with guessed
  hosts. Noted in `PROGRESS.md`.
- **Drafts stay on this computer.** "Opslaan" keeps the draft in the local index (in the account's
  Concepten folder, or "Op deze computer" when there is none) and does not append it to the server.
  Fewer writes to the mailbox, and no half-written draft on the phone.
- **Moves and deletes wait 125 seconds before they reach the server** (the undo time plus 5 s), so
  "Ongedaan maken" never needs the server. Sync skips messages with a waiting move.
- **Error reports wait in a local queue** when sending fails, and go out with the next working
  connection. Still one report per code per day, at most ten a day.
- **mbox import** is built next to PST, DBX, EML and Windows Live Mail, because it costs little and
  covers exports from Gmail Takeout and other programs. It also reads a Thunderbird folder tree.
  Thunderbird stays excluded as a *source we look for*; the app never searches for it.
  **(Martijn: say if mbox should go.)**
- **Release builds unwind on a panic instead of aborting**, so a damaged PST that trips the
  `outlook-pst` crate ends that import with an error, not the whole app.
- **The app icon** is a white envelope on dark blue (`assets/icon.svg`). Martijn may want another.
- **Screen changes from the UX review:** Contactpersonen sits in the top bar (no longer under Meer
  mappen), and Afsluiten moved from the top bar to the bottom of Instellingen (the icon next to the
  clock keeps it too). The list of skipped import folders is copied with "Lijst kopiëren", because
  the app has no command to save a file. Martijn may want these placed differently.

## Hard-rule changes P1–P3 (accepted 16 November 2026)

A consistency review found three places where the hard rules and the plan did not fit together.
Martijn accepted all three; they are now in `CLAUDE.md`.

| # | Rule | Problem | Change made |
| --- | --- | --- | --- |
| P1 | Errors: "never rewrite a server reply" vs Network: the only addresses in a report are the sender's and Martijn's | A reply such as `550 5.1.1 <someone@example.nl>: Recipient address rejected` contains a third person's address. Both rules cannot hold for the emailed report | Add to the Errors rule: "On screen and in 'Kopiëren voor Martijn' the reply is exact. In the emailed report and in logs every address other than the sender's and Martijn's is replaced by `<adres>`; nothing else in the reply changes." |
| P2 | Network: "Besides mail servers, the app contacts only GitHub … and a remote image's server" | The plan also needs the settings lookup (the domain's autoconfig host, Thunderbird's ISPDB, DNS), Google's and Microsoft's sign-in pages, and the WebView2 installer | Add: "…, the settings lookup (the domain's autoconfig host, Thunderbird's ISPDB, DNS), Google's and Microsoft's sign-in pages when the user signs in that way, and Microsoft's WebView2 installer. Anything else is added here first." |
| P3 | Updates: idle means "no message being written, no import running" | The plan also waits for a send in progress | Add "or send": "no message being written, no import or send running" |

## Log

| Date | Decision |
| --- | --- |
| 2026-11-16 | Folder set up for Claude Code from the plan doc and its two reviews |
| 2026-11-16 | Research pass: plan corrected and extended (`research.md`); decisions 14–18 added |
| 2026-11-16 | Search must be fast globally, per mailbox and per folder (Martijn) |
| 2026-11-16 | Tags/labels and contacts in version 1 (Martijn); candidate features listed in `features.md`; decisions 19 and 20 added |
| 2026-11-16 | All research proposals accepted; decisions 2, 5, 10, 13, 17, 20 answered; network hard rule changed (Martijn) |
| 2026-11-16 | Wish button, error reports by email, remote log requests, silent updates through GitHub, Windows notifications, Outlook as reference point (Martijn); decisions 21 and 22 added; decision 4 narrowed to public or private |
| 2026-11-16 | Consistency review: docs fixed; three hard-rule changes proposed (P1–P3) |
| 2026-11-16 | P1–P3 accepted and applied to `CLAUDE.md` (Martijn) |
| 2026-11-16 | Decisions 1, 3, 4, 6–12, 14–16, 19, 21, 22 answered (Martijn); phase 6 dropped |
| 2026-11-16 | Build choices recorded: stack confirmed (decision 6), own SMTP client, no MX/SRV yet, local drafts, delayed server moves, report queue, mbox import, unwind on panic |
| 2026-11-16 | Screen changes from the UX review recorded under Build choices: Contactpersonen in the top bar, Afsluiten in Instellingen, Lijst kopiëren |
