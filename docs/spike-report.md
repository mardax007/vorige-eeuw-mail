# Spike report (phase 1)

Written 17 November 2026 by Claude Code, after phases 1 to 4 and most of phase 5 were built.
The spike and the build ran together: what the spike had to prove was proven by building the real
thing in the intended layout, not by throwaway code. Where a check needs a real mailbox, a family
PC or phone, or a key only Martijn can make, it is marked **not yet** with what is still needed.

Evidence: the tests named below (run on this PC and in CI), `docs/PROGRESS.md` (session log) and
`docs/DECISIONS.md` ("Build choices").

## The checks

| Exit criterion | Result | What was seen |
| --- | --- | --- |
| IMAP: connect, sign in, list, fetch and parse, test server and one real account | Yes on GreenMail; **not yet** on a real account | `tests/greenmail.rs` (folders with special use, headers then bodies, flags, keywords, move, kill during sync, restart) and the Android emulator against GreenMail. A real account needs Martijn (Solcon first, read-only: the client opens with `EXAMINE` and fetches with `BODY.PEEK`). |
| SMTP: sent and received on the test server; a refused send shows the reply verbatim | Yes | Own SMTP client (`smtp.rs`): fake-server tests keep a multi-line 550 reply character for character; `engine_greenmail.rs` sends with undo, a sent copy, accented attachments, replies with In-Reply-To and References, and a message written offline goes once when the connection returns. |
| Settings lookup finds the family's real domains, Solcon included | Partly | Provider table (Solcon `imap.solcon.nl` 993, `mail.solcon.nl` 587 STARTTLS), autoconfig, ISPDB, MX and SRV, tried read-only against real DNS. Not yet confirmed with a real Solcon login. |
| Connections behind antivirus mail scanning, through the Windows store | **Not yet** on Windows | TLS uses `rustls-platform-verifier` everywhere (never a built-in root list). On the Android emulator a self-made test CA in the system store was trusted and removing it gave VEM-IMAP-TLS-01, the same mechanism. Adding a test root to the Windows store is a security setting and needs Martijn's go. |
| Unread counts on the real accounts the same before and after | **Not yet** | Needs a real account. |
| A secret survives a restart and never appears in plain text in a file or a log | Yes | DPAPI file per account (`secrets.rs`); `after_a_restart_mail_reads_offline_and_no_file_holds_the_password` scans every file in the data folder for the password in UTF-8 and UTF-16 after a restart. On Android the file is sealed with a Keystore key (emulator). Decrypting as another Windows user was not tried (needs a second user on this PC). |
| Google sign-in works and still works after eight days | **Not yet** | PKCE and loopback flow built and unit-tested; needs the Google Cloud client Martijn registers. |
| Unsigned installer on a family PC; one update from a GitHub release installs unattended | Installer yes on this PC; **not yet** on a family PC | `npm run tauri build` makes a 9.5 MB per-user NSIS installer that installs and starts. The updater waits until the app is idle (`may_install`, unit test) but needs the updater key and a release. |
| A Windows notification shows on the installed build and opens the app | **Not yet** on an installed build | Built (notification, icon next to the clock, taskbar count); Windows shows notifications properly only for an installed app. |
| The core connects, signs in and stores a secret on an Android phone | Yes on the emulator; **not yet** on a real phone | Android 16 emulator: sign-in, sync, reading, replying, Keystore-sealed password, background mail through Doze and a reboot. |
| Hostile HTML loads nothing, runs nothing, cannot reach the core | Yes | `tests/hostile_html.rs` over `tests/fixtures/hostile.html`, and by hand on Windows and the Android emulator: sanitiser, sandboxed `srcdoc` frame, CSP; links reach the system browser through the shell, `mailto:` opens Write. |
| A real PST read with the right counts, three messages open as `.eml` elsewhere | Yes on four public real PSTs; **not yet** on a family PST | `real_pst_files_import_with_both_readers` (Enron, Aspose sample, two java-libpst files) in CI. A family PST and the comparison with Outlook's counts need Martijn. |
| Search over 100,000 messages at all three levels | Yes | `search_perf`: at most 32 ms on this PC and 46 ms on the Android emulator from an emptied file cache (target 200 ms). The slowest family phone is still to be measured. |
| Capabilities, quota and folder names of each real account | **Not yet** | The help report collects them per mailbox ("Hulprapport kopiëren"); needs real accounts. |
| Sent copy, keywords and connection limit per provider, throwaway accounts only | **Not yet** | Logic built per `sync.md` (search Sent for the Message-ID, append only if missing; keywords only where `PERMANENTFLAGS` has `\*`). Needs a spare mailbox per provider. |
| `spike-report.md` written; decision 6 made | Yes | This file. Martijn left decision 6 to Claude Code on this report. |

## How the crates held up

- **async-imap** (tokio): did the job, including IDLE, once the stop handle of an IDLE was kept
  alive. No QRESYNC; CONDSTORE is enough for now.
- **lettre**: not used. Its error text is not the server's reply verbatim, so a small own SMTP
  client (about 400 lines) keeps every reply line exactly, uses the same rustls connection with
  the system store as IMAP, refuses a missing required STARTTLS and speaks XOAUTH2.
- **mail-parser**, **mail-builder**, **ammonia**: no problems. ammonia needed two extra rules
  (style attributes with `url()` or `expression()`, links to localhost).
- **rusqlite with FTS5**: fast enough with room to spare (see search above). Folder lists needed
  the right indexes (169 ms down to 0.2 ms).
- **outlook-pst**: the weak spot. Version 1.2.0 refused most real messages; it is vendored with six
  small patches (`vendor/outlook-pst/VEM-PATCHES.md`), opened read-only and run under
  `catch_unwind`. `readpst` from libpst ships as the rescue reader for files it cannot read.
- **rustls-platform-verifier**: works on Windows and Android; on Android it needs its Kotlin part
  and a ProGuard rule.
- **Tauri 2**: the shell, tray, notifications, autostart, single instance, updater and Android
  build all work. Android needed care in a few places (no new windows in its WebView, a UI-thread
  dead-lock with the opener plugin, `content://` paths from the save dialog), each fixed.

## Pace of working in Rust

Rust cost time on the edges, not in the mail logic. The borrow checker and async code slowed the
first IMAP and sync code a little; the real time went to third-party gaps (outlook-pst's bugs, the
Android verifier setup, the MSVC library paths on this PC that need `scripts\vcenv.cmd`). In
return the core is one library, tested headless on Windows and reused unchanged on Android, and
the same tests run in CI. Electron would have been no faster on the screens, which are React
either way, and slower on PST, DBX and the Android background service, which need native code.

## Recommendation

Stay with Tauri. Nothing the spike had to prove failed; the open checks are open because they need
a real mailbox, a family PC or phone, or Martijn's keys, not because the stack could not do them.
The Electron fallback (ImapFlow and Nodemailer) is not needed. Decision 6 is recorded in
`DECISIONS.md` under "Build choices".
