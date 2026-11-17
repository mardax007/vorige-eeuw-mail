# Progress

Claude Code updates this file at the end of every session.

## Now

- **Phase:** phases 1 to 4 are built in code and tested on this PC; their exit criteria that need a
  real mailbox, a real PC of a family member, GitHub or Martijn are still open (list below).
  Phase 5 (Android) is not started.
- **Next for Claude Code:** the open build items below, then phase 5 once Martijn says to go on.
- **Martijn, to unblock the rest:**
  1. Create the public GitHub repository, push this folder, and set the Actions secrets
     (`docs/release.md`, "Once, before the first release").
  2. Make the updater key and keep it with its password in the backup (phase 4 task 8).
  3. Register the Google Cloud "Desktop app" client and the Microsoft Entra public client
     (redirect `http://localhost`), and put their ids in the secrets.
  4. Try the app against your own Solcon mailbox and a throwaway Gmail account (read-only first),
     and give Claude Code a real old PST and DBX file to import into a local account.
  5. Take the screenshots for `docs/install.md`.
  6. Say whether the envelope icon and mbox import may stay (`DECISIONS.md`, "Build choices").
  7. Answer question 23 in `DECISIONS.md`: GPL source of the bundled rescue tool, pointer only or
     attached to every release.

## State of the folder

- Code: `crates/vem-core` (mail logic, 145 unit tests plus GreenMail end-to-end and real-file
  import tests),
  `src-tauri` (the shell, 3 tests), `src` (screens, 125 tests), `.github/workflows` (tests, draft release).
  Commands are in `CLAUDE.md`.
- `npm run tauri build` makes `Vorige Eeuw Mail_0.1.0_x64-setup.exe` (9.5 MB, per user; 3.7 MB
  of it is the rescue tool in `rescue\`). The built app starts and shows the Dutch first-run screen.
- MSYS2 with `mingw-w64-ucrt-x86_64-libpst` is installed on this PC (`C:\msys64`);
  `scripts/rescue-tool.mjs` copies the rescue tool from there before every `tauri dev` and `tauri build`.
- Search on 100,000 messages: 10 to 16 ms; a folder's first page 0.2 ms (target 200 ms).

### Built and tested

Accounts by address (provider table, autoconfig, ISPDB, the provider behind the MX host, SRV
records, a guess under the user's domain; DNS hosts outside the user's domain need confirmation), password and Google/Microsoft sign-in (PKCE, loopback), DPAPI secrets file; IMAP sync
with IDLE, offline bodies, flags, star, tags as keywords or Gmail labels, move and delete with undo
(server change delayed 125 s), Verwijderde items and empty; search at three levels; Alle postvakken;
reading view in a sandboxed frame with link warnings and remote images on request; write, reply,
forward, attachments, smaller photos, signature, drafts, Postvak UIT with undo send, sent copies;
contacts learned from mail; error box with exact server reply and `VEM-` code; error reports, wish
button, help report, log requests; mailbox space; notifications, icon next to the clock, autostart,
single instance; unread count of every Postvak IN on the taskbar icon; clicking a notification about
one message opens it; silent updates (download, wait until idle, install quietly, "Bijgewerkt" for a
day); import of PST (four real public files read whole, with mail, folders, attachments, embedded
messages, contacts and the password check — through a patched, vendored `outlook-pst`), DBX, EML
folders, Windows Live Mail folders and mbox, with copies of the
original files, read state, star and tags, into a local account or the mailbox; import from
another added mailbox (read-only; skips Verwijderde items, Ongewenste e-mail, Concepten and
Postvak UIT); Outlook contacts from a PST into the address book; the password of a protected PST is asked and
checked; rescue mode with `readpst` (from MSYS2, shipped in the installer as `rescue\`) for a PST
the normal reader cannot read; uploads into a mailbox honour
`APPENDLIMIT` and `[TOOBIG]`, and stop at 500 MB a day for Gmail; Windows Live Mail headers-only
messages are listed and the screen says its mail came in as read; Live Mail contacts through its
business-card (.vcf) export, explained in four steps on the Import page; a new password or "Opnieuw inloggen" per mailbox in Instellingen; mbox export;
contacts to the phone (QR pairing, encrypted message in the mailbox; the phone side is phase 5).

### Not built yet (open items)

- Windows Live Mail: folder names, read state and stars from `Mail.MSMessageStore` (ESE, through
  `esent.dll` on a copy). Needs a real Live Mail store to build against; until then the import uses
  the folder names on disk and marks all mail read, as phase 3 task 6 allows.
- Rescue mode: proven end to end on a real PST whose header CRC is broken (the normal reader
  rejects it with VEM-IMPORT-READ-01, `readpst` then imports all 71 messages), and on four
  undamaged real PSTs. Not yet run from an installed build. Whether the release should carry the
  GPL source archives is open question 23.
- Help pages on GitHub Pages: generated in `docs/help/`; publishing is a repository setting
  (Pages from `docs/` on main) once the repository exists.

### Never run against real systems yet

Real IMAP/SMTP providers (only GreenMail; the MX/SRV lookup was tried read-only against real DNS),
Google and Microsoft sign-in, a real Live Mail store, the taskbar count and notification click on
an installed build, the updater with a signed release, notifications from an installed build, the
release workflow.
(Real PST and DBX files are now tested — four public PSTs with mail, contacts and a password, and a
real Outlook Express DBX — and CI fetches them so the import tests run on real data; see the
session log for 2026-11-17.)

## Session log

| Date | What was done | What was learned | Next |
| --- | --- | --- | --- |
| 2026-11-16 | Folder set up: CLAUDE.md, plan, decisions, phase files | — | Inventory, then the spike |
| 2026-11-16 | Research pass over the whole plan (five parallel research runs). Rewrote `PLAN.md` and every phase file except 6; added `research.md`, `sync.md`, `screens.md`; decisions 14–18; Martijn's search requirement added; then tags and contacts added to version 1 with `features.md` (candidate list C1–C14); a consistency review then fixed 30 issues, among them read-only spike access to real mailboxes (EXAMINE, BODY.PEEK), a throwaway-account spike task for anything that writes, and link handling without IPC | Solcon has no autoconfig, and its SMTP host and username differ; its mailboxes are 2–5 GB. Antivirus TLS scanning breaks clients with their own root list. `outlook-pst` gives raw MAPI only and has data-loss bugs; MAPI-to-MIME is ours to build. WebView2 puts Tauri's scripts in every frame. New Outlook can export PST. Smart App Control can be switched back on since April 2026. KPN is moving mailboxes from today | Inventory, decisions, then the spike |
| 2026-11-16 | Martijn accepted all proposals. Recorded decisions 2 (React + TypeScript + Vite), 5 (Dutch), 10, 13, 17 (DPAPI), 20 (C1–C4, C6, C7, plus C12). Designed and added: error reports by email, remote log requests through a `log-requests.json` file next to the releases, the "Wens of idee" button (`PLAN.md` section 5); silent updates through GitHub Actions and Releases with a draft tested on Martijn's PC (section 6); Windows notifications, the icon next to the clock and starting with Windows; Outlook as reference point with its Dutch words (`screens.md`). Updated `CLAUDE.md` hard rules (secrets, network, updates, Dutch), `features.md`, `errors.md` (Dutch, `REPORT` area) and phases 1, 2a, 2b, 4, 5. New questions 21 (report address) and 22 (Android updates) | Releases of a private GitHub repository need a login, so silent updates need a public repository for the release files. A silent update of an unsigned app is blocked where Smart App Control is on, which makes signing (decision 14) worth more. Windows shows notifications properly only for an installed app. Android cannot update an app silently outside a store | Inventory, questions 1, 3, 4, 8, then the spike |
| 2026-11-16 | Martijn answered every open question: support as many providers as possible, identifier `tech.mardax.vem`, public repository, GPL-3.0, per-user install, Google sign-in yes, full offline copy, DBX import yes, no import over the internet (phase 6 dropped), old mail kept locally with a copy of the original file and a menu to delete it, mailbox space shown, contacts on the phone through QR pairing and the user's own mailbox (Firebase considered, not chosen), report address set (kept out of the repository), Android "Bijwerken" button. Applied hard-rule changes P1–P3. Updated DECISIONS, PLAN, features, sync, errors, phases 1–6, CLAUDE.md | An admin install would break silent updates (UAC at every update). The report address must stay out of the public repository but is readable from the installer | Martijn's go for phase 1 |
| 2026-11-16 | Phases 1–4 built: Rust core (IMAP, own SMTP client, sync, search, actions, import of PST/DBX/EML/WLM/mbox, reports, contact sync, OAuth), Tauri shell (tray, notifications, autostart, updates), screens in Dutch, CI and release workflows, icon, docs (release, install, help pages, error registry). Installer builds and the app starts | Folder lists needed `id` in their indexes (169 ms to 0.2 ms). `outlook-pst` opens files for writing and can panic, so we open read-only and unwind. GreenMail has no Sent folder. IDLE needs its stop handle kept alive | Martijn's items above; open build items; phase 5 |
| 2026-11-16 | Mailbox-to-mailbox import (GreenMail test); new password and re-sign-in per mailbox; one import at a time, its status kept for the screens; the core sends each mailbox's sign-in type. UX review of all screens (ux-engine) with every finding fixed: the send bar follows the real outbox state, load and open errors no longer look like empty lists, Sluiten keeps a draft that did not save, no double send, sync errors on Alle postvakken, address problems under their field, copy feedback per button, import progress survives leaving the page, Quick Assist text corrected. Contactpersonen moved to the top bar, Afsluiten to the bottom of Instellingen. A code review of the screen changes followed | Quick Assist works the other way round from what the help said: the helper gives the code. A failed list load showed "no messages", which reads as lost mail | Martijn's items above; open build items; phase 5 |
| 2026-11-16 | Unread count on the taskbar icon (drawn overlay, Windows has no badge for desktop apps); a notification about one message opens it. Import uploads honour APPENDLIMIT and TOOBIG and stop at 500 MB a day for Gmail. PST contacts into the address book without overwriting typed ones. Live Mail: headers-only messages listed, and the screen says all came in as read | `outlook-pst` 1.2.0 resolves named-property GUIDs at the wrong offset (it adds 3 twice), so contact e-mail fields are matched by their number only. Windows reports a notification click only while the toast is on screen | Martijn's items above; a real Live Mail store for the ESE reader; phase 5 |
| 2026-11-16 | Live Mail contacts through its .vcf export (one screen, four steps). Code review of the upload limits and imports, all findings fixed: a read or star change on a message waiting for upload goes into the upload, the first upload of a day always goes, Gmail's quota day counted from 08:00 UTC, OVERQUOTA pauses uploads, APPENDLIMIT uses the CRLF size, an empty Live Mail message with a Content-Type is kept, a known contact gets new addresses, a broken vCard line cannot swallow the next card | Gmail's daily upload count follows Pacific time. A queued flag change on a not-yet-uploaded message had nowhere to go and was silently dropped | Martijn's items above; a real Live Mail store for the ESE reader; phase 5 |
| 2026-11-17 | MX and SRV steps of the settings lookup (checked read-only against real DNS); the password of a protected PST asked on the import screen and checked; rescue mode with readpst offered once when a PST cannot be read (app side, tested with a stand-in tool); confirmation text no longer says every unchecked host was guessed | PST passwords protect nothing: the file keeps only a CRC-32. libpst has no official Windows build. Google publishes RFC 6186 SRV records for gmail.com | A Windows readpst for the installer; Martijn's items above; a real Live Mail store; phase 5 |
| 2026-11-17 | MSYS2's libpst installed (Martijn's yes); `scripts/rescue-tool.mjs` collects readpst, its 18 DLLs and their licences into `src-tauri/rescue/`, shipped as `rescue\` by the installer; CI and the release workflow install the package; a test runs the real readpst | readpst opens paths through the ANSI code page and fails under a folder like "Łucja", so it now runs inside its scratch folder with plain names, on a hard link to our copy | Question 23; a real damaged PST; Martijn's items above; phase 5 |
| 2026-11-17 | Imported four public real PSTs (Enron, Aspose sample, java-libpst dist-list and passworded) with the normal reader and readpst side by side. outlook-pst 1.2.0 refused most real messages, so it is now vendored under `vendor/outlook-pst` with six small "accept more files" patches (`VEM-PATCHES.md`), redirected by `[patch.crates-io]`. New test `real_pst_files_import_with_both_readers` over `VEM_REAL_PST_DIR`. The password check verified on a real protected file (refused empty, accepted a matching check) | outlook-pst drops almost every real message: a missing `PtypObject` type arm, an over-strict heap free-count check, distribution lists with no sub-node tree, a missing attachment method, and embedded-message nodes looked up in the wrong sub-node tree (plus a double file-lock dead-lock). readpst double-writes long attachment names as `filename*=` and `filename=` | A truly damaged PST; the real DBX/Live Mail/providers; Martijn's items; phase 5 |
| 2026-11-17 | Imported a real Outlook Express DBX (fillyfoal's `Inbox.dbx`): new test `real_dbx_file_imports` over `VEM_REAL_DBX`. CI now fetches the four PSTs and the DBX into the runner temp and points the import tests at them, so a lost outlook-pst patch fails CI | The DBX fixture's two message-info blocks carry no attributes, so both entries are header-only (no body); our reader reads the signature, item count and index tree right and classifies them correctly | A truly damaged PST; the real Live Mail/providers; Martijn's items; phase 5 |
| 2026-11-17 | Rescue-only path proven on a real PST: flipping one reserved header byte breaks outlook-pst's header CRC but not readpst, so the normal reader fails with VEM-IMPORT-READ-01 (the rescue offer) and readpst imports all 71 messages. New test `a_pst_the_normal_reader_rejects_is_read_by_rescue`, run in CI | outlook-pst checks dwCRCPartial over the header; readpst does not, so a reserved-field flip cleanly separates the two readers | The real Live Mail store/providers/sign-in; installed-build checks; Martijn's items; phase 5 |

## Ideas parked

Things that came up but are not in the current phase. Do not build them without asking.

- A per-sender "always show images" list, as Velo does.
- Substring search (trigram index) on subject and sender, if word search turns out not to be enough.
- QRESYNC through Pimalaya's `io-imap`, once its API settles.
- A KPN entry in the provider table, once KPN publishes the Mail en Cloud server names.
- Automatic tagging rules (left out of version 1 on purpose; see `features.md`, "Out").
- Read aloud (C5), "This is spam" (C8), photo view (C9), conversations (C10), readable invites
  (C11), backup of everything (C14): candidates not chosen yet.
