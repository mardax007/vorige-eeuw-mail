# Progress

Claude Code updates this file at the end of every session.

## Now

- **Phase:** phases 1 to 4 are built in code and tested on this PC; their exit criteria that need a
  real mailbox, a real PC of a family member, GitHub or Martijn are still open (list below).
  Phase 5 (Android) started on 17 November 2026 (Martijn's go): the phone layout of the mail
  screen is built; the Android SDK and NDK are installed (Martijn approved the download) and a
  debug APK builds (`scripts\androidenv.cmd`, CLAUDE.md Commands). Passwords on Android are
  sealed with a key kept in the Android Keystore (task 5) and TLS uses Android's own certificate
  check (task 4). Both ran on the Android 16 emulator against GreenMail (session log, 17 November).
  Sign-in on Android (task 3: Gmail by app password, Microsoft by the browser) and the
  "Bijwerken" button (task 8) are built; Bijwerken ran on the emulator up to Android's own install
  screen. Contacts on the phone (task 9) are built: the phone scans the PC's code with its camera
  and shares the address book through the mailbox (tested on GreenMail; the camera screen ran on
  the emulator, but the emulator cannot read a code). Task 7 is built as Martijn chose (decision
  24, option 2): "Nieuwe mail direct melden" keeps a foreground service watching for new mail with
  the app closed; it ran on the emulator through Doze, a reboot and an update
  (`docs/background-mail.md`). Not yet on a real phone. Task 6: search on 100,000 messages over
  three mailboxes and 30 folders takes at most 46 ms on the emulator at every level, also with its
  file cache emptied (target 200 ms); the slowest family phone is still to be measured. The
  hostile HTML set passes on Android (emulator, 17 November). A second design review of every
  screen (ux-engine, Windows and phone) is done and its findings fixed (session log).
  Martijn answered the last three questions on 17 November 2026: the rescue tool's sources are
  attached to every release (decision 23), every family phone has Google Play (25), and mail kept
  on the PC reaches the paired phone over the home Wi-Fi (26, task 10, `docs/pc-to-phone.md`),
  built and tested between this PC and the emulator. No open questions are left.
- **Next for Claude Code:** nothing can move without Martijn now: every open exit criterion needs a
  real mailbox, a family PC or phone, or GitHub secrets. Every criterion that can be proven on this
  PC, GreenMail and the emulator is ticked in `docs/phases/` with its test and date, and
  `docs/spike-report.md` is written (17 November 2026).
- **Martijn, to unblock the rest:**
  1. Done 18 November 2026: every Actions secret from `docs/release.md` is set except
     `VEM_MICROSOFT_CLIENT_ID` (Microsoft skipped, step 3). The public repository
     `mardax007/vorige-eeuw-mail` exists since 17 November 2026, with the Tests workflow green
     and the help pages on GitHub Pages.
  2. The updater key exists since 18 November 2026 (`%USERPROFILE%\.tauri\vem-updater.key` and
     `.pub`). Martijn copied both files and the password into his backup (18 November 2026).
  3. Google: the Cloud project `vorige-eeuw-mail` (External, consent screen "Vorige Eeuw Mail")
     and its "Desktop app" client exist since 17 November 2026. Its id and secret are in the Actions
     secrets `VEM_GOOGLE_CLIENT_ID` and `VEM_GOOGLE_CLIENT_SECRET` (17 November 2026). The project is "In production", unverified
     (one warning screen, at most 100 users ever), with the help pages as home page and
     `docs/privacy.md` as privacy policy, on the authorised domain `mardax007.github.io`. An
     Android-type client needs the Android signing key first (step 8).
     Microsoft: skipped, nobody in `family-inventory.md` uses Outlook.com, and the registration
     needs an Azure sign-up with a card check (`PLAN.md`, Microsoft).
  4. Try the app against your own Solcon mailbox and a throwaway Gmail account (read-only first),
     and give Claude Code a real old PST and DBX file to import into a local account.
  5. Take the screenshots for `docs/install.md`.
  6. Done 18 November 2026: the envelope icon and mbox import stay (`DECISIONS.md`, "Build choices").
  7. The first release ran on 18 November 2026: the draft `v0.1.0` has the installer and its
     signature, `latest.json`, `vorige-eeuw-mail.apk`, `android.json` and the 14 source archives
     of the rescue tool (decision 23), all signed with the updater key. On 18 November 2026 Claude
     Code checked the installer's signature against `vem-updater.key.pub` (good), installed the
     draft on this PC (per user, `%LOCALAPPDATA%\Vorige Eeuw Mail`, rescue tool included) and
     started it: the window opens. The log has `VEM-UPDATE-CMD-01` "Could not fetch a valid release
     JSON from the remote", which is expected while the release is a draft (the
     `releases/latest/download/latest.json` link only works once it is published). Still to do:
     install the APK on your phone (no phone was connected over USB), use both for a day, then
     publish (`docs/release.md`, "Every release", steps 4 to 6). After publishing, the next start
     of the PC app should log no update failure. Later that day the draft was rebuilt in place from
     6fcefd4 (the new look, decision 27; run 44333621209, both jobs green), its installer checked
     against the updater key again (good) and installed over the first one on this PC; it starts.
     It is still numbered 0.1.0; from the next run on the workflow numbers builds itself
     (decision 28), so that one will be `0.1.<run number>` and an update for every installed app.
  8. The Android signing key exists since 18 November 2026
     (`%USERPROFILE%\.android\vem-upload.jks`, alias `vem`), is in the secrets and, with its
     password, in Martijn's backup (18 November 2026). Still to do: the Android-type Google
     client (step 3).
  9. Phase 5 task 8: the free limited-distribution account at Google before 2027, with each family
     phone added.
  10. On each family phone's make, switch on "Nieuwe mail direct melden", answer Android's battery
      question with Toestaan, leave the phone for an hour and send it a mail: makers' battery rules
      decide whether the service survives (dontkillmyapp.com lists extra steps per make).
  11. Fill in which family phones are Android, and their make, in `docs/family-inventory.md`.
  12. Pair a real phone with your PC once ("Telefoon koppelen" on both) and check a contact goes
      both ways; the emulator cannot read a QR code.
  13. Run the search speed test on the slowest family phone (USB debugging on, then the two
      commands in `CLAUDE.md`, "The same on a phone or the emulator", with the arm64 target). It
      writes 100 MB in the phone's temporary folder for about ten minutes; the last line says
      "slowest search". Send Claude Code the output.
  14. Mail from the PC on the phone (task 10): with the phone paired and both on the home Wi-Fi,
      import some old mail on your PC and wait two minutes with the app open on the phone. Check
      that Windows asked nothing about the firewall and that the folders show on the phone. (The two leftover firewall
      rules named "tcplisten.exe" are removed, 18 November 2026.)
  15. A debug APK shows a white page for about 8 seconds at start on the emulator before the
      list; check whether the release APK on a real phone does the same.

## State of the folder

- Code: `crates/vem-core` (mail logic, 189 unit tests plus 20 GreenMail end-to-end tests, real-file
  import and damaged-file tests),
  `src-tauri` (the shell, 6 tests), `src` (screens, 173 tests), `.github/workflows` (tests, draft release).
  Commands are in `CLAUDE.md`.
- `npm run tauri build` makes `Vorige Eeuw Mail_0.1.0_x64-setup.exe` (9.5 MB, per user; 3.7 MB
  of it is the rescue tool in `rescue\`). The built app starts and shows the Dutch first-run screen.
- MSYS2 with `mingw-w64-ucrt-x86_64-libpst` is installed on this PC (`C:\msys64`);
  `scripts/rescue-tool.mjs` copies the rescue tool from there before every `tauri dev` and `tauri build`.
- Search on 100,000 messages over three mailboxes and 30 folders, at all three levels: at most
  32 ms on this PC and 46 ms on the Android emulator from an emptied file cache; a folder's first
  page 0.3 ms (target 200 ms).

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
  the folder names on disk and marks all mail read, as phase 3 task 6 allows. A self-made store was
  considered (17 November) and not built: public sources document only the store's Folders table
  (libyal esedb-kb, HiraokaHyperTools/ExploreWLMFolders), not the table that holds read state and
  stars, so a test store would only test our own guess.
- A PST cut short (a copy broken off at 50 or 75 %) cannot be read by either reader: outlook-pst
  stops with VEM-IMPORT-READ-01 and readpst says "Could not get root record", also when the file
  is padded back to its recorded length, because the PST's index pages lie near its end. The user
  gets the error and the advice to ask Martijn. Damage inside the file is handled (see the log).
- Rescue mode: proven end to end on a real PST whose header CRC is broken (the normal reader
  rejects it with VEM-IMPORT-READ-01, `readpst` then imports all 71 messages), and on four
  undamaged real PSTs. Not yet run from an installed build. Whether the release should carry the
  GPL source archives is open question 23.

### Never run against real systems yet

Real IMAP/SMTP providers (only GreenMail; the MX/SRV lookup was tried read-only against real DNS),
Google and Microsoft sign-in, a real Live Mail store, the taskbar count and notification click on
an installed build, the updater with a signed release, notifications from an installed build, the
release workflow (its Android job has never run: it needs the signing key secrets), Google and
Microsoft sign-in on a phone, the last step of "Bijwerken" (Android's "Toestaan van deze bron"
and the update itself; on the emulator the run stopped there, since that switch is a security
setting), new mail in the background on a real phone (and the 15-minute alarm firing), and
reading a real QR code with a phone's camera (the emulator has no Google Play
services, which the scanner needs to decode).
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
| 2026-11-17 | GitHub: public repository `mardax007/vorige-eeuw-mail` (GPL-3.0), Tests workflow and Pages on; first runs green, real-file tests included. Damaged data, on self-made damage (Martijn: "corrupt data to see recov"): `Store::open` refuses a damaged `index.sqlite` (`PRAGMA quick_check`, VEM-STORE-READ-01, file left as it is); a mailbox message whose `.eml` was removed or emptied is downloaded again when opened (GreenMail test), a local one reports its loss; a damaged password file gives VEM-STORE-SECRET-01; `import/corrupt_tests.rs` feeds truncated, overwritten and bit-flipped copies of the empty and Enron PST, the real and a self-made 40-message DBX, an mbox and an EML folder to the import (each must end within 2 minutes, leave the file unchanged, and import what it can or stop with a VEM-IMPORT-READ code). Phase 5 started: phone layout of the mail screen (one part at a time below 640 px) | Three import faults found and fixed: a non-DBX file "imported 0 messages" (now refused, or listed as skipped in a folder); a DBX cut inside a message block lost the whole recovery scan (now 26 of 26 whole messages back); a damaged mbox start and a cut "From " line were dropped silently or became an empty message (now listed). Bit flips and overwritten runs in the Enron PST lose at most a few messages (68 to 71 of 71). A PST cut short is lost to both readers (open items). The Live Mail message table is undocumented | Android SDK and NDK (Martijn's OK for the download), then the APK; phase 5 tasks 7 and 8 meanwhile |
| 2026-11-17 | Android build: SDK command-line tools, platform 36, build-tools, NDK 29 in `%LOCALAPPDATA%\Android\Sdk` (Gradle added platform 37 and build-tools 36 itself; the SDK licence was accepted as part of Martijn's go), four Rust Android targets, `tauri android init`. Desktop-only parts (tray, autostart, single instance, updater, folder picker, taskbar badge) behind `cfg(desktop)`; `tauri.android.conf.json` leaves the Windows rescue tool out of the APK. Keystore-sealed password files on Android (same file layout and damage handling as DPAPI; sealing tested on Windows). `rustls-platform-verifier` Android part: Kotlin component from its Maven archive, ProGuard rule, `net::init_android()` at start. Debug APK (arm64) builds. | `JAVA_HOME` must be a real `jdk-*` folder (the `latest` link breaks Gradle); Gradle 9.6.1 runs on JDK 26. A source edit during a running Android build breaks that build. The verifier's README says group `rustls`, its archive uses `org.rustls`. | Emulator run (approval for the download), then sign-in on Android |
| 2026-11-17 | Emulator run (Martijn approved the download): AVD `vem-test`, Android 16 x86_64, with GreenMail behind a self-made test CA and a test autoconfig host (all in `Mail-scratch`, outside the repository). Checked on the emulator: the app starts and Android's certificate check works (a real HTTPS settings lookup; GreenMail refused with VEM-IMAP-TLS-01 once the test CA was removed); adding the GreenMail mailbox; reading, replying (In-Reply-To right), new mail after a force-stop; the password file sealed with the Keystore key (no plain text in the file or the preferences); a damaged password file shows VEM-STORE-SECRET-01, and a new password in Instellingen repairs it and mail comes in again with a notification. Fixed on the way: the screen ran under the status bar (Android 15+ edge to edge, now padded by the window insets); notifications need Android's one-time question (asked at start and after adding a mailbox, `capabilities/mobile.json`); a fetch that failed before the screens listened was never shown (the stored error is now read at start); the old error stayed after a working new password (the core now clears it at once); "Starten met Windows" is hidden on a phone (`AppInfo.isPhone`); a reply's heading said "Nieuwe e-mail" (now "Antwoord schrijven") | `rustls-platform-verifier` uses Android's `AndroidCAStore`, so certificates the user installed are trusted on Android too (as the Windows store does). The x86_64 APK for the emulator: `--target x86_64` in the APK command | Task 3 (sign-in on Android), task 8, task 7 proposal; Android backup switched off on Martijn's answer (`DECISIONS.md`) |
| 2026-11-17 | Android backup off (Martijn). Task 3: sign-in on Android; Gmail takes an app password on a phone, Microsoft keeps the browser sign-in, the page after sign-in says to open the app again. Task 8: "Bijwerken" on a phone. The app reads `android.json` next to `latest.json` (15 s after start, then every six hours), shows "Er is een nieuwe versie", downloads the APK from `https://github.com/` only, checks it with the updater key (minisign, the same key as Windows) and opens Android's install screen through a FileProvider (`ApkInstaller.kt`, JNI). Release workflow: an Android job builds the signed APK, signs the update and adds the APK and `android.json` to the draft (not yet run); `scripts/android-release.mjs` writes `android.json`; release notes now come from `release-notes.md`. `docs/release.md` and `docs/install.md` have the Android steps. On the emulator, with a throwaway key and a test HTTPS server standing in for github.com: offer shown, 674 MB debug APK fetched and checked, Android's install screen opened | Latest.json's notes were written when the workflow ran, so editing the draft's text (the old step 3) never reached the apps. Tauri's Android context is the Application object, whose class comes from the framework's loader, so app classes must be loaded through `Context.getClassLoader()`. With `MSYS_NO_PATHCONV=1` set, `cmd //c` from Git Bash opens an idle shell and the build silently does nothing. On a touch screen a tapped button keeps its hover look, which made the disabled "Bijwerken" unreadable | Task 7 proposal, task 9, task 6; Martijn's items |
| 2026-11-17 | Task 9, contacts on the phone: the phone side of "Telefoon koppelen" ("Code scannen" with the camera through Tauri's official barcode-scanner plugin, a full-screen Stoppen over the camera picture, a camera "no" explained with "Instellingen openen", "Koppeling verbreken"); the core takes the PC's code (`pair_from_code`, new codes VEM-PAIR-READ-01 and VEM-PAIR-FIND-01) and links the phone's own mailbox with the same address; a GreenMail test pairs two devices and sends changes both ways, and a new code locks the old key out. A change made in the same second as the other device's now still wins (`changed_at` always moves forward). The contacts folder "Vorige Eeuw Mail" no longer shows in the folder list (`sync.md`), and the screens reload the folder list when a folder comes or goes. A phone names its local drafts group "Op deze telefoon". Task 7: proposal written (`docs/background-mail.md`), open question 24 | On the emulator the camera screen and Stoppen work. Stoppen showed "[object Object]" at first: Tauri's mobile plugins reject with a plain `{message}` object, which `toVemError` now reads. A Stoppen in the first seconds left the camera on, because the plugin switches it on only once Android's camera service has started; the app now stops it again after 1.5, 5 and 10 s. The plugin's decoder is Google ML Kit, which needs Google Play services and queues usage events for `firebaselogging.googleapis.com`; that uploader is removed in the app's manifest (the log then says "Transport backend 'cct' is not registered"); question 25 | Task 6; Martijn's items 10–12 |
| 2026-11-17 | Task 7 built, Martijn's choice option 2 (decision 24): "Nieuwe mail direct melden" in Instellingen (phone only, off by default) starts an own `specialUse` foreground service in a separate process `:mail` that runs the Rust core headless (`android_mail.rs`, Kotlin `MailService`, `MailAlarm`, `MailBoot`, `MailCore`). Only one process works on the mailboxes at a time: two lock files (`vem_core::handoff`, unit tests), the engine can pause and start again (`Engine::pause`, GreenMail test), and the app's process works only while its screen is visible. On a phone the app's process now ends with its screen. Notification texts reach the service through the index. Emulator: hand-over both ways, a notification from the service, mail in forced deep Doze, restart after a reboot and after a reinstall, switch off stops everything, reopening shows the screen (no white page) with the mail the service fetched | Tauri's `app_local_data_dir` on Android is the app's data folder itself (no identifier added, unlike its doc comment). Android raises a foreground service's lowest-importance channel to "low". `BOOT_COMPLETED` and `MY_PACKAGE_REPLACED` may start a `specialUse` service (only some types are barred). Tauri's notification plugin starts the app's process at boot for its own receiver; that process runs no setup and takes no turn | Task 6; Martijn's item 10 (try it on each family make) |
| 2026-11-17 | Task 6, search speed on a phone: `search_perf` now spreads 100,000 messages over three mailboxes with ten folders each and times all three levels ("alle mail", "dit postvak", "deze map"), each query twice, failing on anything over 200 ms; `VEM_PERF_DIR` keeps its index for a second run. `scripts/ndklinkers.cmd` sets the NDK compilers for plain cargo on Android targets; the test builds for arm64 and x86_64 and runs on the emulator through `adb` (CLAUDE.md, Commands). Emulator: building the index 363 s; searches in a fresh process at most 19 ms, from an emptied file cache at most 46 ms, and in the same process after a minute's rest at most 21 ms. PC: at most 32 ms | Searching in the first seconds after writing 100 MB took up to 473 ms on the emulator, also for a word with no hits; after a minute's rest or in a new process it was 21 ms, so the time went to the disk writing the new index back, not to the query. A real phone after a big first sync can see the same for a moment. `adb push` does not always keep the execute bit | Hostile HTML set on Android; Martijn's item 13 (the slowest family phone) |
| 2026-11-17 | Hostile HTML set on Android: `tests/fixtures/hostile.html` (every trick from phase 1) and `tests/hostile_html.rs` through the sanitiser; on the emulator the whole chain (sanitiser, sandboxed frame, CSP) loaded nothing, ran nothing and reached nothing, remote pictures only after "Afbeeldingen tonen". Fixed on the way: style attributes could keep `url()`/`expression()` and links to localhost passed the sanitiser; a link tapped on a phone did nothing (Android's WebView has no new windows: on a phone the frame now lets a tapped link through to the shell) and then froze the app (the shell opened the browser on Android's UI thread; now off it). Verified on the emulator: a web link opens the browser, `mailto:` opens Write with its subject, no freeze. Second design review of every screen (ux-engine, three reports) and its findings fixed: Android back key (`src/api/backButton.ts`: an open message, the folder screen, a page, Write, an open contact, setup steps; never while a contact is edited); open message on a phone with the sender first and buttons two by two, no Afdrukken; attachments saved on Android (they silently were not: `tauri-plugin-fs` writes the `content://` place) with "… is opgeslagen."; ConfirmDialog keeps focus inside and returns it; folders behind Meer mappen stay open when one is shown; labels: Enter makes one, Klaar keeps a typed one; contact editor, setup and password problems under their field with focus there, no double save; loading and failed states for search, contacts and import sources; sticky Versturen bar; signature saved on leaving the field; settings spring back when not stored; phone texts in Help; the report note waits for a mailbox; paired phone shown on the PC | Android's WebView ignores `target=_blank` in a sandboxed frame and has no popup window, so the shell's navigation handler never saw the tap. wry runs that handler on Android's UI thread, where the opener plugin waits for the same thread: a dead-lock that Android reports as "not responding". The save dialog's answer on Android is a `content://` address that `into_path()` refuses | Martijn's items; questions 23, 25, 26 |
| 2026-11-17 | Martijn's answers to 23, 25 and 26. 23: `scripts/rescue-sources.mjs` fetches the MSYS2 source archive of every package the rescue tool comes from (package base read from pacman's database) and the release workflow attaches them to the draft. 25: Google Play on every phone, so the QR scanner stays. 26, task 10: mail kept on the PC reaches the paired phone over the home Wi-Fi (`docs/pc-to-phone.md`, module `vem-core/src/lan`): the PC sends a sealed UDP find packet, the phone answers and listens on TCP, the PC connects and sends what the phone lacks, framed with XChaCha20-Poly1305 under a session key from the pairing key. The phone's copy follows the PC (moves, read, star, deletions); on the phone "Oude mail ophalen" offers only another mailbox. Codes `VEM-LAN-COPY-01`, `VEM-LAN-SPACE-01`. CLAUDE.md network rules amended | Windows' firewall asks nothing for a program that only sends and connects out; a listening PC would have shown the question to every family member. A unicast reply to a subnet broadcast can come in from WSL's adapter, so the PC sends to every private network's broadcast address. On the emulator (UDP redirect and TCP forward) 41 messages in two copied local accounts arrived with folder, date and star, and the second look said "up to date" without a connection | Martijn: a real phone on the home Wi-Fi (list item 14) |
| 2026-11-17 | Closed the exit criteria that can be proven here, each with a test: the import result links to the imported mail and names its file; "Martijn krijgt hier vanzelf bericht van" shows only when a report was really queued; help pages say so; SPACE-02 gives the numbers; address book export as `.vcf`; reports, wish and log answers followed to a GreenMail inbox; imported mail found at three search levels; an import cut off halfway goes on without doubles; a sync killed at five moments loses and doubles nothing; offline reading after a restart with no file holding the password; accented attachments and reply headers; tags between two devices; a message written offline goes once when the connection returns; Postvak UIT shows the exact reply; the update waits for idle; no literal text in any screen; print, text size and address suggestions; PST categories as tags. `docs/spike-report.md` written and the proven criteria ticked in `docs/phases/` | Two real faults found by the new tests: tag keywords lost their capitals (another device showed "familie" for "Familie"), and a message that failed to connect waited its full five-minute retry after the connection was back; both fixed. Doorsturen dropped the original's attachments (found while checking the forward criterion); it now takes them along. GreenMail keeps keywords from earlier runs in the same mailbox, so tests use names unique per run | Martijn's items above |
| 2026-11-17 | Google Cloud project `vorige-eeuw-mail` with its "Desktop app" client, published (unverified, personal use); `docs/privacy.md` on GitHub Pages as its privacy policy. Third design review (ux-engine) of the screens changed since the second, every finding fixed: contact files on a phone (save and add did nothing), the phone's copy line ends when a copy breaks off and a full phone is said once, Doorsturen does not open without its attachments, the mailbox-full import message in Dutch and right on a phone, smaller texts and buttons, a line while Doorsturen makes the attachments ready. Contact files saved and added on the Android emulator through the system's save and pick screens | Android's pick and save screens give a content:// address everywhere, not only for attachments; every file the user picks must go through the fs plugin | Martijn: the rest of the list above |
| 2026-11-18 | Test mail in bulk: `scripts/seed-greenmail.mjs` (with `seed-data.mjs`) fills five GreenMail mailboxes in Solcon, Gmail, Outlook and plain styles with about 3,500 made-up Dutch messages over three years (nested folders, `$vem_` labels, threads with In-Reply-To and References, read and unread, star, answered, drafts, junk, trash, photos and PDFs, newsletters with remote images, the three warning samples) and writes 250 made-up contacts as vCard files; local hosts only (CLAUDE.md, Commands). On the emulator the seeded `jan@example.test` synced: folder roles from the Dutch and Gmail names, nested folders as "Werk / Projecten", labels in the folder list, and the "geld van een nieuwe afzender" warning on the sample | GreenMail's folder separator is "."; `guess_role` already covers every provider name used. Keywords without the `$vem_` prefix (Thunderbird's `$label1`, `NonJunk`) are rightly ignored | Martijn: try the five mailboxes on the PC (needs the test CA in his own Windows store, his call) |
| 2026-11-18 | The new look (decision 27, Martijn's yes): tokens rewritten in `base.css`, then `mail.css`, `layout.css`, `forms.css` and the mail frame's own style. Checked in Chrome at 1400 px and at 375 px (list, open message, Instellingen, Write, text size 32: no sideways scroll, buttons wrap) | The proposal's contrast numbers hold when computed (lowest text pair 7.17:1, `--accent` on `--hover`). The settings page's "Terug naar de mail" sits outside the sheet, so on a phone it needed its own side margin | Martijn: look at it on the PC and phone |

## Ideas parked

Things that came up but are not in the current phase. Do not build them without asking.

- Tapping the service's "Nieuwe e-mail" notification on a phone opens the app at Postvak IN;
  opening that one message, as on Windows, needs the message id passed into the app's screen
  (decision 24 work, 17 November 2026).

- On a phone, "Oude mail ophalen" (import) is offered as on the PC, although old mail files rarely
  live on a phone, and "Telefoon koppelen" sits under Meer mappen. Hide import on a phone, or keep
  it for a mailbox-to-mailbox import? Ask Martijn.
- Our own block scanner for a PST cut short (read the data blocks without the index pages, the way
  scanpst does). Neither outlook-pst nor readpst can do it; only worth it if a family PST turns out
  to be cut.

- A per-sender "always show images" list, as Velo does.
- Substring search (trigram index) on subject and sender, if word search turns out not to be enough.
- QRESYNC through Pimalaya's `io-imap`, once its API settles.
- A KPN entry in the provider table, once KPN publishes the Mail en Cloud server names.
- Automatic tagging rules (left out of version 1 on purpose; see `features.md`, "Out").
- Read aloud (C5), "This is spam" (C8), photo view (C9), conversations (C10), readable invites
  (C11), backup of everything (C14): candidates not chosen yet.
